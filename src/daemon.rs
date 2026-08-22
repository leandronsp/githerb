//! The two commands that stay up: the review surface with the runner
//! alongside, and the runner on its own. Wiring only: the lock, the watcher
//! that wakes the loops, the signal that stops them.

use std::fmt;
use std::io::{self, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use app::Store;
use web::{Server, Surface, Wakeup, Watcher};

/// How often the watcher asks git whether anything moved.
const PROBE_EVERY: Duration = Duration::from_millis(500);

/// How long the runner alongside the review surface waits when nothing moves.
const RUNNER_EVERY: Duration = Duration::from_secs(2);

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
fn shutdown_flag() -> Result<Arc<AtomicBool>, Failure> {
    let flag = Arc::new(AtomicBool::new(false));
    let flipped = Arc::clone(&flag);
    ctrlc::set_handler(move || flipped.store(true, Ordering::Relaxed))?;
    Ok(flag)
}

/// Where the running commentary of a long-lived command goes.
fn say(line: &str) {
    let _ignored = writeln!(io::stderr(), "{line}");
}

/// `githerb review`: the pages, and unless told otherwise the runner alongside
/// them, so the thing you leave open all day is the thing that answers.
pub fn review(
    proposal: Option<&str>,
    port: u16,
    open: bool,
    run: bool,
    out: &mut dyn Write,
) -> Result<(), Failure> {
    let store = Store::at(".")?;
    let author = app::Identity::detect(store.repo());
    let watcher = web::watching(&store, PROBE_EVERY);
    let surface = Surface::new(store.clone(), author, watcher.clone(), Box::new(say));
    let server = Server::bind(&format!("127.0.0.1:{port}"))?;
    let shutdown = shutdown_flag()?;

    let mut url = format!("http://{}", server.addr());
    if let Some(id) = proposal {
        url.push_str("/p/");
        url.push_str(id);
    }
    writeln!(out, "reviewing at {url}")?;
    if open {
        launch(&url);
    }

    let alongside = if run {
        answer_alongside(&store, &watcher, &shutdown)
    } else {
        None
    };

    let outcome = web::serve(&surface, &server, Arc::clone(&shutdown));
    shutdown.store(true, Ordering::Relaxed);
    watcher.stop();
    if let Some(thread) = alongside {
        // A join that fails is a runner thread that panicked; there is nothing
        // left to clean up and nothing to report beyond what it already said.
        let _ignored = thread.join();
    }
    Ok(outcome?)
}

/// Start the runner on its own thread, or say why not: a second runner on the
/// same repository is refused by the lock, and the pages are served anyway.
fn answer_alongside(
    store: &Store,
    watcher: &Watcher,
    shutdown: &Arc<AtomicBool>,
) -> Option<JoinHandle<()>> {
    let lock = match runner::Lock::acquire(store.repo().git_dir()) {
        Ok(lock) => lock,
        Err(err) => {
            say(&format!("not answering the log: {err}"));
            return None;
        }
    };
    if let Err(err) = runner::prune_leftovers(store.repo()) {
        say(&format!("pruning worktrees: {err}"));
    }
    let runner = runner::Runner::new(
        store.clone(),
        store.repo().root().to_path_buf(),
        app::Identity::runner(),
        Box::new(say),
    );
    let mut subscription = watcher.subscribe();
    let shutdown = Arc::clone(shutdown);
    Some(std::thread::spawn(move || {
        let _held = lock;
        if let Err(err) = runner.recover() {
            say(&format!("recovering: {err}"));
        }
        let mut wait = |budget: Duration| matches!(subscription.wait(budget), Wakeup::Changed);
        if let Err(err) = runner.run(&mut wait, RUNNER_EVERY, &shutdown) {
            say(&format!("runner: {err}"));
        }
    }))
}

/// Open the browser on the page, and say nothing if that is not possible:
/// the address was printed, and that is the contract.
fn launch(url: &str) {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ignored = std::process::Command::new(opener)
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
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
        Box::new(say),
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
    let watcher = web::watching(&store, PROBE_EVERY);
    let mut subscription = watcher.subscribe();
    let mut wait = |budget: Duration| matches!(subscription.wait(budget), Wakeup::Changed);
    let outcome = runner.run(&mut wait, every, &shutdown);
    watcher.stop();
    Ok(outcome?)
}
