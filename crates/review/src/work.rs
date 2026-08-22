//! What an agent is doing, and who asked for it.
//!
//! Two lines bracket a task: started, then finished or failed with one line
//! about why. Nobody writes a status field; whether someone is on a proposal
//! is what the log adds up to, which is the only version of the answer that
//! cannot go stale.
//!
//! [`activity`] folds a slice of work records. The aggregate hands it the
//! records on the head revision and nothing else, deliberately: a rebase that
//! failed on revision one must not leave revision four reading as failed
//! forever, which is exactly what the Go build did.

use std::fmt;

use crate::chunk::{Field, one_line};
use crate::errors::{Error, Result};
use crate::identity::{Author, Sha};
use crate::timestamp::Timestamp;

/// What an agent was asked to do. The set is closed because a task nobody can
/// run is a record nobody can act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Task {
    /// Answer the open notes and leave a revision.
    Apply,
    /// Move the work onto a target that ran ahead.
    Rebase,
    /// Run what the repository declares.
    Check,
}

impl Task {
    /// Read a task off the wire or a command line.
    ///
    /// # Errors
    ///
    /// A task this build does not run.
    pub fn parse(raw: &str) -> Result<Self> {
        match raw.trim() {
            "apply" => Ok(Self::Apply),
            "rebase" => Ok(Self::Rebase),
            "check" => Ok(Self::Check),
            _ => Err(Error::UnknownTask(raw.to_owned())),
        }
    }

    /// The word the wire format uses.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Apply => "apply",
            Self::Rebase => "rebase",
            Self::Check => "check",
        }
    }
}

impl fmt::Display for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where a task is: it started, it finished, or it did not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Phase {
    /// Somebody has it in hand.
    Started,
    /// It was carried to the end.
    Finished,
    /// It was tried and did not work.
    Failed,
    /// A claim handed back without an answer, which is what a runner killed
    /// mid-job leaves behind. Not a failure: the task stays available.
    Cleared,
}

impl Phase {
    /// Read a phase off the wire or a command line.
    ///
    /// # Errors
    ///
    /// A phase this build does not know.
    pub fn parse(raw: &str) -> Result<Self> {
        match raw.trim() {
            "started" => Ok(Self::Started),
            "finished" => Ok(Self::Finished),
            "failed" => Ok(Self::Failed),
            "cleared" => Ok(Self::Cleared),
            _ => Err(Error::UnknownPhase(raw.to_owned())),
        }
    }

    /// The word the wire format uses.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Finished => "finished",
            Self::Failed => "failed",
            Self::Cleared => "cleared",
        }
    }
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One line of what an agent did, appended as it happens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Work {
    revision: Sha,
    task: Task,
    phase: Phase,
    agent: Author,
    note: Option<String>,
    at: Timestamp,
}

impl Work {
    /// The only way to build one.
    ///
    /// # Errors
    ///
    /// A note over its ceiling, or carrying more than one line.
    pub fn new(
        revision: Sha,
        task: Task,
        phase: Phase,
        agent: Author,
        note: Option<&str>,
        at: Timestamp,
    ) -> Result<Self> {
        let note = match note {
            None => None,
            Some(raw) => {
                let said = one_line(Field::Note, raw)?;
                if said.is_empty() { None } else { Some(said) }
            }
        };
        Ok(Self {
            revision,
            task,
            phase,
            agent,
            note,
            at,
        })
    }

    /// The head the work was about.
    #[must_use]
    pub fn revision(&self) -> &Sha {
        &self.revision
    }

    /// What was being done.
    #[must_use]
    pub fn task(&self) -> Task {
        self.task
    }

    /// How far it got.
    #[must_use]
    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// Who was doing it.
    #[must_use]
    pub fn agent(&self) -> &Author {
        &self.agent
    }

    /// The one line it left behind, usually why it stopped.
    #[must_use]
    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }

    /// When, to the second, in UTC.
    #[must_use]
    pub fn at(&self) -> Timestamp {
        self.at
    }
}

/// A person handing the open notes to an agent.
///
/// It carries nothing but the revision it was asked about, because everything
/// the agent needs to read is already in the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dispatch {
    revision: Sha,
    author: Author,
    at: Timestamp,
}

impl Dispatch {
    /// The only way to build one. Nothing here can be refused.
    #[must_use]
    pub fn new(revision: Sha, author: Author, at: Timestamp) -> Self {
        Self {
            revision,
            author,
            at,
        }
    }

    /// The head it was asked about.
    #[must_use]
    pub fn revision(&self) -> &Sha {
        &self.revision
    }

    /// Who asked.
    #[must_use]
    pub fn author(&self) -> &Author {
        &self.author
    }

    /// When, to the second, in UTC.
    #[must_use]
    pub fn at(&self) -> Timestamp {
        self.at
    }
}

/// What the work log adds up to right now.
///
/// One agent works on a proposal at a time, so the last line wins and there is
/// nothing to merge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Activity {
    phase: Phase,
    task: Task,
    agent: Author,
    since: Timestamp,
    note: Option<String>,
}

impl Activity {
    /// Whether an agent has this proposal in hand.
    #[must_use]
    pub fn working(&self) -> bool {
        self.phase == Phase::Started
    }

    /// Whether the last thing tried did not work.
    #[must_use]
    pub fn failed(&self) -> bool {
        self.phase == Phase::Failed
    }

    /// Whether nobody is on it. A finished task and a handed-back claim are
    /// both idle.
    #[must_use]
    pub fn idle(&self) -> bool {
        !self.working() && !self.failed()
    }

    /// Where the last task got to.
    #[must_use]
    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// What is being done, or what failed.
    #[must_use]
    pub fn task(&self) -> Task {
        self.task
    }

    /// Who is doing it.
    #[must_use]
    pub fn agent(&self) -> &Author {
        &self.agent
    }

    /// When the current phase began.
    #[must_use]
    pub fn since(&self) -> Timestamp {
        self.since
    }

    /// The line the agent left, usually the reason it stopped.
    #[must_use]
    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }
}

/// Fold a work log in the order it happened: the latest record wins, and among
/// records sharing a moment the one written last does.
///
/// An empty log is nobody on it, which is [`None`] and not a zero value.
#[must_use]
pub fn activity(work: &[Work]) -> Option<Activity> {
    let mut latest: Option<&Work> = None;
    for record in work {
        if latest.is_none_or(|best| record.at >= best.at) {
            latest = Some(record);
        }
    }
    latest.map(|record| Activity {
        phase: record.phase,
        task: record.task,
        agent: record.agent.clone(),
        since: record.at,
        note: record.note.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &str = "9f6c1e2a3b4d5e6f708192a3b4c5d6e7f8091a2b";

    fn work(phase: Phase, note: Option<&str>, unix: i64) -> Result<Work> {
        Work::new(
            Sha::parse(HEAD)?,
            Task::Rebase,
            phase,
            Author::parse("githerb-run")?,
            note,
            Timestamp::from_unix(unix),
        )
    }

    #[test]
    fn a_task_and_a_phase_are_closed_sets() -> Result<()> {
        assert_eq!(Task::parse(" apply ")?, Task::Apply);
        assert_eq!(Phase::parse("cleared")?, Phase::Cleared);
        assert_eq!(
            Task::parse("dream"),
            Err(Error::UnknownTask("dream".to_owned()))
        );
        assert_eq!(Task::parse(""), Err(Error::UnknownTask(String::new())));
        assert_eq!(
            Phase::parse("sleeping"),
            Err(Error::UnknownPhase("sleeping".to_owned()))
        );
        assert_eq!(Phase::parse(""), Err(Error::UnknownPhase(String::new())));
        Ok(())
    }

    #[test]
    fn a_note_over_its_ceiling_is_refused() -> Result<()> {
        let long = "x".repeat(141);
        assert_eq!(
            work(Phase::Failed, Some(&long), 10),
            Err(Error::TooLong {
                field: Field::Note,
                chars: 141,
                ceiling: 140
            })
        );
        assert_eq!(work(Phase::Failed, Some("  "), 10)?.note(), None);
        Ok(())
    }

    #[test]
    fn an_empty_log_is_nobody_on_it() {
        assert_eq!(activity(&[]), None);
    }

    #[test]
    fn a_started_record_reads_as_working() -> Result<()> {
        let log = vec![work(Phase::Started, None, 10)?];
        let activity = activity(&log).ok_or(Error::NoAuthor)?;
        assert!(activity.working());
        assert_eq!(activity.task(), Task::Rebase);
        assert_eq!(activity.agent().as_str(), "githerb-run");
        Ok(())
    }

    #[test]
    fn a_finished_record_reads_as_idle_and_both_lines_stay() -> Result<()> {
        let log = vec![
            work(Phase::Started, None, 10)?,
            work(Phase::Finished, Some("done"), 20)?,
        ];
        let activity = activity(&log).ok_or(Error::NoAuthor)?;
        assert!(activity.idle());
        assert_eq!(log.len(), 2);
        Ok(())
    }

    #[test]
    fn records_folded_out_of_order_still_fold_by_timestamp() -> Result<()> {
        let log = vec![
            work(Phase::Failed, Some("conflicts in a.txt"), 20)?,
            work(Phase::Started, None, 10)?,
        ];
        let activity = activity(&log).ok_or(Error::NoAuthor)?;
        assert!(activity.failed());
        assert_eq!(activity.note(), Some("conflicts in a.txt"));
        Ok(())
    }

    #[test]
    fn a_cleared_claim_reads_as_idle() -> Result<()> {
        let log = vec![
            work(Phase::Started, None, 10)?,
            work(
                Phase::Cleared,
                Some("the runner that claimed this is gone"),
                20,
            )?,
        ];
        let activity = activity(&log).ok_or(Error::NoAuthor)?;
        assert!(activity.idle());
        assert!(!activity.failed());
        Ok(())
    }
}
