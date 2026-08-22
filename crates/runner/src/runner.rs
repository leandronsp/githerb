//! The loop, and the sequence of every job it does.
//!
//! One pass reads the log once, works out what it asks for, and does it one
//! job at a time. An agent job is minutes of a machine and somebody's money,
//! so nothing here is concurrent and nothing is retried: a failure is written
//! down and stands until a person hands the proposal over again.
//!
//! Every job is bracketed by two lines in the log, `started` and then
//! `finished` or `failed`. That is what makes a claim visible to the browser
//! and to the next runner, and it is why a job that is killed still ends in a
//! record rather than in silence.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use app::{Config, Snapshot, Store};
use review::{Author, Phase, Proposal, ProposalId, Task};

use crate::error::Error;
use crate::jobs::{Job, pending};

pub mod tasks;

/// What a claim left by a process that is gone is cleared with.
const CLAIM_GONE: &str = "the runner that claimed this is gone";

/// The domain's ceiling for a one line note, less the room an ellipsis needs.
const NOTE_CEILING: usize = 137;

/// The smallest slice of the floor the loop will give up in one wait, so that
/// a waiter which answers without waiting cannot turn the floor into a spin.
const QUANTUM: Duration = Duration::from_millis(10);

/// One repository, and the loop that answers its log.
pub struct Runner {
    store: Store,
    config_root: PathBuf,
    author: Author,
    say: Box<dyn Fn(&str) + Send + Sync>,
}

impl Runner {
    /// Point a runner at a store.
    ///
    /// `config_root` is where `.githerb.toml` lives, read again on every pass
    /// so that a check or an agent added to it takes effect without a restart.
    /// `author` is what the work log is signed with, usually
    /// [`app::Identity::runner`]. `say` is where the running commentary goes.
    #[must_use]
    pub fn new(
        store: Store,
        config_root: PathBuf,
        author: Author,
        say: Box<dyn Fn(&str) + Send + Sync>,
    ) -> Self {
        Runner {
            store,
            config_root,
            author,
            say,
        }
    }

    /// Hand back the claims of a runner that is gone, and say how many.
    ///
    /// The caller holds the repository lock, so anything still `started` was
    /// left by a process that died. It is cleared rather than failed: the task
    /// was never carried to an answer, so it stays available.
    ///
    /// # Errors
    ///
    /// The log could not be read, or the record could not be written.
    pub fn recover(&self) -> Result<usize, Error> {
        let snapshot = self.store.snapshot()?;
        let mut cleared = 0;

        for proposal in snapshot.open() {
            let Some(activity) = proposal.activity() else {
                continue;
            };

            if !activity.working() {
                continue;
            }

            self.report(
                proposal.id(),
                activity.task(),
                Phase::Cleared,
                Some(CLAIM_GONE),
            )?;
            self.say(&format!(
                "{}: {} was left claimed, handing it back",
                proposal.id(),
                activity.task()
            ));

            cleared += 1;
        }

        Ok(cleared)
    }

    /// One pass: read the log, work out what it asks for, and do it.
    ///
    /// Returns how many jobs were performed, whether or not they succeeded: a
    /// job that failed is a job that was answered, and the answer is in the
    /// log.
    ///
    /// # Errors
    ///
    /// The log could not be read, or a record could not be written. A job that
    /// fails is recorded, not returned.
    pub fn once(&self, shutdown: &AtomicBool) -> Result<usize, Error> {
        // Nothing of ours is in flight when a pass begins, because a pass does
        // one job at a time and finishes it. Holding the lock is what makes
        // "anything still claimed belongs to a dead process" true.
        self.recover()?;

        let config = self.config();
        let required = app::required(&config)?;
        let snapshot = self.store.snapshot()?;
        let stale = self.stale(&snapshot);
        let mut done = 0;

        for job in pending(snapshot.proposals(), &stale, &required) {
            if shutdown.load(Ordering::Relaxed) {
                break;
            }

            self.work(&config, &job, shutdown)?;
            done += 1;
        }

        Ok(done)
    }

    /// Pass, wait, pass again, until `shutdown` flips.
    ///
    /// `wait` is how this sleeps: it is given a budget, blocks for at most
    /// that long, and answers whether something woke it. It must consume the
    /// wake-up it reports, and it must return when the thing it waits on stops
    /// or this loop cannot notice a shutdown. Two passes are never closer
    /// together than `every`, however often the log moves.
    ///
    /// # Errors
    ///
    /// Never: a pass that refuses is said out loud and the loop goes on, since
    /// a runner that gives up on one bad read stops answering the log at all.
    pub fn run(
        &self,
        wait: &mut dyn FnMut(Duration) -> bool,
        every: Duration,
        shutdown: &AtomicBool,
    ) -> Result<(), Error> {
        while !shutdown.load(Ordering::Relaxed) {
            let began = Instant::now();

            if let Err(refused) = self.once(shutdown) {
                self.say(&format!("{refused}"));
            }

            let mut left = every.saturating_sub(began.elapsed());

            while !left.is_zero() && !shutdown.load(Ordering::Relaxed) {
                let waited = Instant::now();
                wait(left);
                let spent = waited.elapsed();

                // A waiter that answers instantly would spin the floor away.
                // Holding the line here costs a well behaved one nothing and
                // stops a badly behaved one from burning a core.
                if spent < QUANTUM {
                    std::thread::sleep(QUANTUM.saturating_sub(spent));
                }

                left = left.saturating_sub(waited.elapsed());
            }
        }

        Ok(())
    }

    // --- one job ---

    /// Claim a job, do it, and write down how it went.
    fn work(&self, config: &Config, job: &Job, shutdown: &AtomicBool) -> Result<(), Error> {
        let proposal = self.store.load(job.id())?;

        self.say(&format!("{}: {}, {}", job.id(), job.task(), job.why()));
        self.report(job.id(), job.task(), Phase::Started, None)?;

        match self.perform(config, job.task(), &proposal, shutdown) {
            Ok(note) => {
                self.say(&format!("{}: {} done, {note}", job.id(), job.task()));
                self.report(job.id(), job.task(), Phase::Finished, Some(&note))
            }
            Err(refused) => {
                let why = refused.to_string();
                self.say(&format!("{}: {} failed: {why}", job.id(), job.task()));
                self.report(job.id(), job.task(), Phase::Failed, Some(&why))
            }
        }
    }

    /// What each task actually does.
    fn perform(
        &self,
        config: &Config,
        task: Task,
        proposal: &Proposal,
        shutdown: &AtomicBool,
    ) -> Result<String, Error> {
        match task {
            Task::Apply => self.apply(config, proposal, shutdown),
            Task::Rebase => self.rebase(config, proposal, shutdown),
            Task::Check => self.check(config, proposal),
        }
    }
    // --- the small print ---

    /// Which proposals the target branch has run ahead of.
    ///
    /// This is the one question the log cannot answer: it is a fact about two
    /// branches, not about anything anybody wrote down. A proposal whose
    /// target git cannot resolve is passed over rather than taken as behind,
    /// so a deleted branch costs that proposal its jobs and nobody else's.
    fn stale(&self, snapshot: &Snapshot) -> HashSet<ProposalId> {
        let mut behind = HashSet::new();

        for proposal in snapshot.open() {
            match self.behind(proposal) {
                Ok(true) => {
                    behind.insert(proposal.id().clone());
                }
                Ok(false) => {}
                Err(refused) => self.say(&format!("{}: {refused}", proposal.id())),
            }
        }

        behind
    }

    /// Whether the target has commits this proposal was not cut from.
    fn behind(&self, proposal: &Proposal) -> Result<bool, Error> {
        let repo = self.store.repo();
        let tip = repo.head_of(&proposal.target().git_ref())?;
        let common = repo.merge_base(&tip, proposal.head().sha().as_str())?;

        Ok(common != tip)
    }

    /// What the repository declares right now.
    ///
    /// A file that cannot be read is said out loud and treated as declaring
    /// nothing, which stops one bad edit from wedging the loop.
    fn config(&self) -> Config {
        match Config::load(&self.config_root) {
            Ok(config) => config,
            Err(refused) => {
                self.say(&format!("{refused}"));

                Config::default()
            }
        }
    }

    /// Write one line of the work log.
    fn report(
        &self,
        id: &ProposalId,
        task: Task,
        phase: Phase,
        note: Option<&str>,
    ) -> Result<(), Error> {
        // The note is cut here rather than trusted. A record refused for being
        // one character too long would leave the job looking like it never
        // finished, which is worse than a sentence that ends early.
        let note = note.map(cut);

        app::report(
            &self.store,
            &self.author,
            app::now(),
            id,
            task.as_str(),
            phase.as_str(),
            note.as_deref(),
        )?;

        Ok(())
    }

    /// Say something, if anybody is listening.
    fn say(&self, line: &str) {
        (self.say)(line);
    }
}

/// One line, cut to what a record holds, with an ellipsis where it was cut.
fn cut(note: &str) -> String {
    let line = first_line(note);

    if line.chars().count() <= NOTE_CEILING {
        return line.to_owned();
    }

    let kept: String = line.chars().take(NOTE_CEILING).collect();

    format!("{kept}...")
}

/// The first line of something that may have several.
pub(crate) fn first_line(text: &str) -> &str {
    text.trim().lines().next().unwrap_or_default().trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_note_longer_than_the_ceiling_is_cut_not_refused() {
        let long = "x".repeat(400);

        let note = cut(&long);

        assert_eq!(note.chars().count(), 140);
        assert!(note.ends_with("..."));
    }

    #[test]
    fn a_note_that_fits_is_left_alone() {
        assert_eq!(
            cut("  answered 2, no code changed  "),
            "answered 2, no code changed"
        );
    }

    #[test]
    fn only_the_first_line_of_a_reason_is_written_down() {
        assert_eq!(
            cut("the agent stopped: no api key\nusage: claude"),
            "the agent stopped: no api key"
        );
    }
}
