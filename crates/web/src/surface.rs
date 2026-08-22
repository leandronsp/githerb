//! The review surface: what the routes are served from.
//!
//! One value holds everything a request needs and everything worth keeping
//! between requests: the store, who is working here, the watcher every open
//! page waits on, and two caches.
//!
//! The caches are the whole performance story. A snapshot of the log is kept
//! and refreshed only when the repository's fingerprint moved, so a tab that
//! is merely open costs one cheap git process a tick rather than four per
//! tick per tab. A parsed diff is immutable for a pair of commits, so it is
//! parsed once and shared by every render of it.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use app::{Config, Snapshot, Store};
use patch::Patch;
use review::{Author, CheckName, Proposal, ProposalId};

use crate::error::Error;
use crate::http::{Handler, Server};
use crate::response::Response;
use crate::watch::Watcher;

/// How long a quiet event stream waits before it says something so the client
/// knows it is still there.
pub const HEARTBEAT: Duration = Duration::from_secs(15);

/// Where the server says what a browser must not be told: the text of a git
/// refusal, a read that failed under an open stream.
pub type Log = Box<dyn Fn(&str) + Send + Sync>;

/// Everything the routes are served from.
pub struct Surface {
    store: Store,
    author: Author,
    watcher: Watcher,
    say: Log,
    stopped: Arc<AtomicBool>,
    heartbeat: AtomicU64,
    snapshot: Mutex<Option<Arc<Snapshot>>>,
    patches: Mutex<HashMap<(String, String), Arc<Patch>>>,
}

impl Surface {
    /// Build the surface. `say` is where a failure nobody should see the
    /// details of goes: the browser is told git refused, the terminal is told
    /// what git said.
    #[must_use]
    pub fn new(store: Store, author: Author, watcher: Watcher, say: Log) -> Arc<Self> {
        Arc::new(Self {
            store,
            author,
            watcher,
            say,
            stopped: Arc::new(AtomicBool::new(false)),
            heartbeat: AtomicU64::new(as_millis(HEARTBEAT)),
            snapshot: Mutex::new(None),
            patches: Mutex::new(HashMap::new()),
        })
    }

    /// The one function the server calls for every request.
    #[must_use]
    pub fn handler(self: &Arc<Self>) -> Handler {
        let surface = Arc::clone(self);

        Arc::new(move |request| crate::routes::route(&surface, &request))
    }

    /// How often a quiet stream sends a heartbeat. Tests turn it down so they
    /// do not wait fifteen seconds to see one.
    pub fn heartbeat(&self, every: Duration) {
        self.heartbeat.store(as_millis(every), Ordering::Relaxed);
    }

    // --- what a handler asks for ---

    /// Who is working here, which is what every record is signed with.
    pub(crate) fn author(&self) -> &Author {
        &self.author
    }

    /// The log, read again only when the repository moved.
    pub(crate) fn snapshot(&self) -> app::Result<Arc<Snapshot>> {
        let mut held = lock(&self.snapshot);

        let fresh = match held.as_ref() {
            Some(known) => self.store.snapshot_if_changed(known.fingerprint())?,
            None => Some(self.store.snapshot()?),
        };

        if let Some(fresh) = fresh {
            *held = Some(Arc::new(fresh));
        }

        match held.as_ref() {
            Some(snapshot) => Ok(Arc::clone(snapshot)),
            // The branch above filled it. Reading again is the honest answer
            // to a state that cannot happen rather than a panic that says so.
            None => self.store.snapshot().map(Arc::new),
        }
    }

    /// One proposal, or nothing under that name.
    pub(crate) fn proposal(&self, id: &ProposalId) -> app::Result<Option<Proposal>> {
        Ok(self.snapshot()?.get(id).cloned())
    }

    /// The parsed diff of a proposal from wherever the reader is measuring.
    ///
    /// A diff is immutable for a pair of commits, so this is parsed once per
    /// pair and every later render is a hash lookup.
    pub(crate) fn patch(&self, proposal: &Proposal, since: Option<u32>) -> app::Result<Arc<Patch>> {
        let from = app::origin(proposal, since)?;
        let to = proposal.head().sha();
        let key = (from.as_str().to_owned(), to.as_str().to_owned());

        if let Some(known) = lock(&self.patches).get(&key) {
            return Ok(Arc::clone(known));
        }

        let raw = self.store.repo().diff(from.as_str(), to.as_str())?;
        let parsed = Arc::new(patch::parse(&raw)?);
        lock(&self.patches).insert(key, Arc::clone(&parsed));

        Ok(parsed)
    }

    /// What the repository requires before anything lands. Read per request:
    /// it is one small file, and a review session outlives the edit that adds
    /// a check to it.
    pub(crate) fn required(&self) -> app::Result<Vec<CheckName>> {
        app::required(&Config::load(self.store.repo().root())?)
    }

    /// The store, for the use cases that write.
    pub(crate) fn store(&self) -> &Store {
        &self.store
    }

    /// Probe now: called after every write so the page moves at once rather
    /// than at the next tick.
    pub(crate) fn poke(&self) {
        self.watcher.poke();
    }

    /// A stream's place in the sequence of changes.
    pub(crate) fn subscribe(&self) -> crate::watch::Subscription {
        self.watcher.subscribe()
    }

    /// How long a stream waits before a heartbeat.
    pub(crate) fn quiet(&self) -> Duration {
        Duration::from_millis(self.heartbeat.load(Ordering::Relaxed))
    }

    /// Whether the server is going away, which is what ends an open stream.
    pub(crate) fn stopped(&self) -> bool {
        self.stopped.load(Ordering::Relaxed)
    }

    /// Say something to whoever is running the server, never to the browser.
    pub(crate) fn log(&self, message: &str) {
        (self.say)(message);
    }

    /// The answer to a write: nothing to say, and every open page told to
    /// look again at once.
    pub(crate) fn wrote<T>(&self, done: app::Result<T>) -> Response {
        match done {
            Ok(_done) => {
                self.poke();
                Response::no_content()
            }
            Err(err) => self.refuse(&err),
        }
    }

    // --- refusals ---

    /// What the browser is told about a refusal.
    ///
    /// The domain's own sentence is the message, because it was written for a
    /// person. Anything from git is not: its text carries the command line
    /// and the record that was on it, so the browser gets a sentence and the
    /// terminal gets the detail.
    pub(crate) fn refuse(&self, err: &app::Error) -> Response {
        match err {
            app::Error::NotFound(id) => Response::plain(404, &format!("proposal {id}: not found")),
            app::Error::Git(_) | app::Error::Io(_) => {
                self.log(&format!("refused: {err}"));
                Response::plain(500, "git refused that")
            }
            app::Error::Review(_)
            | app::Error::Patch(_)
            | app::Error::Config(_)
            | app::Error::Description(_)
            | app::Error::NotARevision(_)
            | app::Error::NoSuchRevision(_)
            | app::Error::Log { .. }
            | app::Error::NotFastForward(_)
            | app::Error::CheckKilled(_)
            | app::Error::CheckFailed { .. } => Response::bad_request(err.to_string()),
        }
    }
}

/// Serve until `shutdown` is set, then end every open stream.
///
/// The listener stops accepting within a tick of the flag; the streams are
/// still parked on the watcher, so stopping it is what lets their threads
/// return and the process exit without waiting for a browser tab.
///
/// # Errors
///
/// Fails when the listener itself breaks.
pub fn serve(
    surface: &Arc<Surface>,
    server: &Server,
    shutdown: Arc<AtomicBool>,
) -> Result<(), Error> {
    let result = server.serve(surface.handler(), shutdown);

    surface.stopped.store(true, Ordering::Relaxed);
    surface.watcher.stop();

    result
}

/// A watcher on the repository's fingerprint: one probe for the whole
/// process, whatever else subscribes to it.
#[must_use]
pub fn watching(store: &Store, every: Duration) -> Watcher {
    let store = store.clone();

    Watcher::new(move || store.fingerprint().unwrap_or_default(), every)
}

/// A poisoned lock still holds the last value, and a cache that stops
/// answering because one request panicked is worse than a stale entry.
fn lock<T>(what: &Mutex<T>) -> MutexGuard<'_, T> {
    what.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A duration as whole milliseconds, saturating rather than wrapping.
fn as_millis(every: Duration) -> u64 {
    u64::try_from(every.as_millis()).unwrap_or(u64::MAX)
}
