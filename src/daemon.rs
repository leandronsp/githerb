//! The two commands that stay up: the runner on its own, and (soon) the
//! review surface with the runner alongside. Wiring only: the lock, the
//! watcher that wakes the loop, the signal that stops it.

use std::fmt;
use std::io::{self, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use app::Store;
use web::{Wakeup, Watcher};

/// How often the watcher asks git whether anything moved.
const PROBE_EVERY: Duration = Duration::from_millis(500);

/// Why a long-running command stopped before it was asked to.
#[derive(Debug)]
pub enum Failure {
    /// The repository or the use cases refused.
    App(app::Error),
    /// The runner refused, or could not take the lock.
    Runner(runner::Error),
    /// The server could not be bound or broke.
    Web(web::Error),
    /// The signal handler could not be installed.
    Signal(ctrlc::Error),
    /// Writing to the terminal failed.
    Io(io::Error),
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Failure::App(err) => write!(f, "{err}"),
            Failure::Runner(err) => write!(f, "{err}"),
            Failure::Web(err) => write!(f, "{err}"),
            Failure::Signal(err) => write!(f, "installing the signal handler: {err}"),
            Failure::Io(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for Failure {}

impl From<app::Error> for Failure {
    fn from(err: app::Error) -> Self {
        Failure::App(err)
    }
}

impl From<review::Error> for Failure {
    fn from(err: review::Error) -> Self {
        Failure::App(err.into())
    }
}

impl From<app::ConfigError> for Failure {
    fn from(err: app::ConfigError) -> Self {
        Failure::App(err.into())
    }
}

impl From<runner::Error> for Failure {
    fn from(err: runner::Error) -> Self {
        Failure::Runner(err)
    }
}

impl From<web::Error> for Failure {
    fn from(err: web::Error) -> Self {
        Failure::Web(err)
    }
}

impl From<ctrlc::Error> for Failure {
    fn from(err: ctrlc::Error) -> Self {
        Failure::Signal(err)
    }
}

impl From<io::Error> for Failure {
    fn from(err: io::Error) -> Self {
        Failure::Io(err)
    }
}

/// A flag that SIGINT and SIGTERM flip, so every loop here has one thing to
/// look at.
pub fn shutdown_flag() -> Result<Arc<AtomicBool>, Failure> {
    let flag = Arc::new(AtomicBool::new(false));
    let flipped = Arc::clone(&flag);
    ctrlc::set_handler(move || flipped.store(true, Ordering::Relaxed))?;
    Ok(flag)
}

/// One watcher over the repository, probed on its own thread; everything that
/// wants to know when the log moved subscribes to it.
pub fn watcher(store: &Store) -> Watcher {
    let probe = store.clone();
    Watcher::new(move || probe.fingerprint().unwrap_or_default(), PROBE_EVERY)
}

/// `githerb run`: the runner on its own, for a machine that serves no pages.
pub fn run(once: bool, every: Duration, out: &mut dyn Write) -> Result<(), Failure> {
    let store = Store::at(".")?;
    let root = store.repo().root().to_path_buf();
    let _lock = runner::Lock::acquire(store.repo().git_dir())?;
    runner::prune_leftovers(store.repo())?;

    let runner = runner::Runner::new(
        store.clone(),
        root.clone(),
        app::Identity::runner(),
        Box::new(|line| {
            let _ignored = writeln!(io::stderr(), "{line}");
        }),
    );
    let shutdown = shutdown_flag()?;
    runner.recover()?;

    if once {
        if runner.once(&shutdown)? == 0 {
            writeln!(out, "nothing to do")?;
        }
        return Ok(());
    }

    writeln!(
        out,
        "watching {} every {}",
        root.display(),
        crate::cli::Every(every)
    )?;
    let watcher = watcher(&store);
    let mut subscription = watcher.subscribe();
    let mut wait = |budget: Duration| matches!(subscription.wait(budget), Wakeup::Changed);
    let outcome = runner.run(&mut wait, every, &shutdown);
    watcher.stop();
    Ok(outcome?)
}
