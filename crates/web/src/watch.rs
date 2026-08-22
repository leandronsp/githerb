//! One thread that watches one value, and everybody else waiting on it.
//!
//! Git has nothing to subscribe to, so somebody has to look. The point of
//! putting the looking here is that it happens once: one thread probes, and
//! every open page waits on a generation counter it compares with what it last
//! saw. Twenty tabs cost one probe, not twenty.
//!
//! A handler that has just written calls [`Watcher::poke`], so the page moves
//! as soon as the write lands rather than at the next tick. The tick is the
//! floor, not the mechanism.
//!
//! Subscribers and the probe thread share one condition variable. Everybody
//! waits in a loop on their own predicate and every signal is a `notify_all`,
//! which is what makes that safe.

use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

/// Why a subscriber woke up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wakeup {
    /// The watched value moved since this subscription last looked.
    Changed,
    /// Nothing moved before the timeout ran out.
    Timeout,
    /// The watcher was stopped and will not report again.
    Stopped,
}

struct State {
    value: String,
    generation: u64,
    poked: bool,
    stopped: bool,
}

struct Inner {
    state: Mutex<State>,
    signal: Condvar,
}

impl Inner {
    /// A poisoned lock still holds the last value, and a watcher that stops
    /// answering because a subscriber panicked is worse than a stale probe.
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A background probe whose result subscribers wait on.
///
/// Clone it as widely as you like: every clone is the same probe, the same
/// thread and the same waiters.
#[derive(Clone)]
pub struct Watcher {
    inner: Arc<Inner>,
}

impl Watcher {
    /// Probe once, then keep probing every `every` until stopped.
    ///
    /// The first probe runs here, before this returns, so [`Watcher::current`]
    /// never reports a value nobody ever measured.
    pub fn new(probe: impl Fn() -> String + Send + 'static, every: Duration) -> Self {
        let inner = Arc::new(Inner {
            state: Mutex::new(State {
                value: probe(),
                generation: 0,
                poked: false,
                stopped: false,
            }),
            signal: Condvar::new(),
        });

        let worker = Arc::clone(&inner);
        thread::spawn(move || watch(&worker, &probe, every));

        Self { inner }
    }

    /// Wait on changes from here on. What already happened is not news.
    #[must_use]
    pub fn subscribe(&self) -> Subscription {
        Subscription {
            seen: self.inner.lock().generation,
            inner: Arc::clone(&self.inner),
        }
    }

    /// The last value the probe returned.
    #[must_use]
    pub fn current(&self) -> String {
        self.inner.lock().value.clone()
    }

    /// Probe now rather than at the next tick.
    pub fn poke(&self) {
        self.inner.lock().poked = true;
        self.inner.signal.notify_all();
    }

    /// Stop the thread and wake every waiter with [`Wakeup::Stopped`].
    ///
    /// This returns as soon as it has said so; the thread ends at its next
    /// wakeup, which the signal makes immediate.
    pub fn stop(&self) {
        self.inner.lock().stopped = true;
        self.inner.signal.notify_all();
    }
}

/// One reader's place in the sequence of changes.
pub struct Subscription {
    inner: Arc<Inner>,
    seen: u64,
}

impl Subscription {
    /// Block until the value moves past what this subscription last saw.
    ///
    /// Returns [`Wakeup::Changed`] having caught up, [`Wakeup::Timeout`] when
    /// nothing moved in time, or [`Wakeup::Stopped`] once the watcher is done.
    /// A change that landed between two calls is still reported, so a caller
    /// that answers a change and comes back cannot miss the next one.
    pub fn wait(&mut self, timeout: Duration) -> Wakeup {
        let deadline = Instant::now() + timeout;
        let mut state = self.inner.lock();

        loop {
            if state.stopped {
                return Wakeup::Stopped;
            }
            if state.generation != self.seen {
                self.seen = state.generation;
                return Wakeup::Changed;
            }

            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Wakeup::Timeout;
            }

            let (guard, _outcome) = self
                .inner
                .signal
                .wait_timeout(state, remaining)
                .unwrap_or_else(PoisonError::into_inner);
            state = guard;
        }
    }
}

/// Probe on the tick, on a poke, and never while holding the lock.
fn watch<P: Fn() -> String>(inner: &Inner, probe: &P, every: Duration) {
    loop {
        {
            let mut state = inner.lock();
            loop {
                if state.stopped {
                    return;
                }
                if state.poked {
                    break;
                }

                let (guard, outcome) = inner
                    .signal
                    .wait_timeout(state, every)
                    .unwrap_or_else(PoisonError::into_inner);
                state = guard;

                if outcome.timed_out() {
                    break;
                }
            }
            state.poked = false;
        }

        let probed = probe();

        let mut state = inner.lock();
        if state.stopped {
            return;
        }
        if probed != state.value {
            state.value = probed;
            state.generation += 1;
            drop(state);
            inner.signal.notify_all();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    const PATIENT: Duration = Duration::from_secs(5);
    const BRIEF: Duration = Duration::from_millis(50);
    const NEVER: Duration = Duration::from_secs(3600);

    /// A probe backed by a value the test moves. `NEVER` as the period means
    /// nothing happens unless the test pokes it, so no test waits on a tick.
    fn watched(value: &str) -> (Watcher, Arc<Mutex<String>>) {
        let source = Arc::new(Mutex::new(value.to_owned()));
        let probe = Arc::clone(&source);

        (
            Watcher::new(move || probe.lock().unwrap().clone(), NEVER),
            source,
        )
    }

    fn set(source: &Arc<Mutex<String>>, value: &str) {
        value.clone_into(&mut source.lock().unwrap());
    }

    // --- probing ---

    #[test]
    fn the_first_probe_happens_before_new_returns() {
        let (watcher, _source) = watched("refs/heads/main abc");

        assert_eq!(watcher.current(), "refs/heads/main abc");
        watcher.stop();
    }

    #[test]
    fn a_poke_probes_now_and_reports_the_change() {
        let (watcher, source) = watched("one");
        let mut reader = watcher.subscribe();

        set(&source, "two");
        watcher.poke();

        assert_eq!(reader.wait(PATIENT), Wakeup::Changed);
        assert_eq!(watcher.current(), "two");
        watcher.stop();
    }

    #[test]
    fn a_probe_that_finds_nothing_new_wakes_nobody() {
        let (watcher, _source) = watched("one");
        let mut reader = watcher.subscribe();

        watcher.poke();

        assert_eq!(reader.wait(BRIEF), Wakeup::Timeout);
        watcher.stop();
    }

    #[test]
    fn a_change_is_reported_once_and_not_again() {
        let (watcher, source) = watched("one");
        let mut reader = watcher.subscribe();

        set(&source, "two");
        watcher.poke();

        assert_eq!(reader.wait(PATIENT), Wakeup::Changed);
        assert_eq!(reader.wait(BRIEF), Wakeup::Timeout);
        watcher.stop();
    }

    #[test]
    fn a_change_between_two_waits_is_not_lost() {
        let (watcher, source) = watched("one");
        let mut reader = watcher.subscribe();

        assert_eq!(reader.wait(BRIEF), Wakeup::Timeout);
        set(&source, "two");
        watcher.poke();

        assert_eq!(reader.wait(PATIENT), Wakeup::Changed);
        watcher.stop();
    }

    // --- many readers ---

    #[test]
    fn every_subscriber_wakes_on_one_change() {
        let (watcher, source) = watched("one");
        let mut first = watcher.subscribe();
        let mut second = watcher.subscribe();

        set(&source, "two");
        watcher.poke();

        assert_eq!(first.wait(PATIENT), Wakeup::Changed);
        assert_eq!(second.wait(PATIENT), Wakeup::Changed);
        watcher.stop();
    }

    #[test]
    fn a_subscription_starts_from_now_not_from_the_last_change() {
        let (watcher, source) = watched("one");
        let mut early = watcher.subscribe();

        set(&source, "two");
        watcher.poke();
        assert_eq!(early.wait(PATIENT), Wakeup::Changed);

        // The change has landed. A page opening now waits for the next one
        // rather than replaying it.
        let mut late = watcher.subscribe();

        assert_eq!(late.wait(BRIEF), Wakeup::Timeout);
        watcher.stop();
    }

    // --- stopping ---

    #[test]
    fn stopping_wakes_a_waiting_subscriber() {
        let (watcher, _source) = watched("one");
        let mut reader = watcher.subscribe();
        let (woke, waited) = mpsc::channel();

        thread::spawn(move || woke.send(reader.wait(PATIENT)));
        watcher.stop();

        assert_eq!(waited.recv().unwrap(), Wakeup::Stopped);
    }

    #[test]
    fn a_subscriber_that_arrives_after_the_stop_is_told_so() {
        let (watcher, _source) = watched("one");

        watcher.stop();

        assert_eq!(watcher.subscribe().wait(PATIENT), Wakeup::Stopped);
    }
}
