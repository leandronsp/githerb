//! What a proposal adds up to: the derived answers both surfaces read.
//!
//! Nothing here is stored. Whether a proposal is open, whether an agent is on
//! it, whether it is waiting for one, which notes block a landing: all of it is
//! a fold over the records, computed on demand, so it cannot disagree with the
//! log.

use std::fmt;

use super::{Proposal, Revision, State};
use crate::branch::Branch;
use crate::check::{Check, CheckName};
use crate::chunk::Chunk;
use crate::comment::Comment;
use crate::errors::{Error, Result};
use crate::identity::{ProposalId, RecordId, Sha};
use crate::rationale::Rationale;
use crate::reply::Reply;
use crate::timestamp::Timestamp;
use crate::work::{self, Activity, Phase, Work};

/// The shortest true thing that can be said about the checks on the head
/// revision, for a column in a list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckSummary {
    /// Nothing has run on the head.
    None,
    /// Everything that ran said yes.
    Passing,
    /// This many said no.
    Failed(usize),
}

impl fmt::Display for CheckSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => f.write_str("no checks"),
            Self::Passing => f.write_str("passing"),
            Self::Failed(count) => write!(f, "{count} failed"),
        }
    }
}

impl Proposal {
    /// What the proposal is called.
    #[must_use]
    pub fn id(&self) -> &ProposalId {
        &self.id
    }

    /// What a person calls it.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The branch it lands on.
    #[must_use]
    pub fn target(&self) -> &Branch {
        &self.target
    }

    /// The commit it was cut from.
    #[must_use]
    pub fn base(&self) -> &Sha {
        &self.base
    }

    /// Where it is in its life.
    #[must_use]
    pub fn state(&self) -> State {
        self.state
    }

    /// When it opened.
    #[must_use]
    pub fn opened_at(&self) -> Timestamp {
        self.opened_at
    }

    /// When it landed or was given up on, when the log said so.
    #[must_use]
    pub fn ended_at(&self) -> Option<Timestamp> {
        self.ended_at
    }

    /// Every attempt, oldest first.
    #[must_use]
    pub fn revisions(&self) -> Vec<&Revision> {
        std::iter::once(&self.first).chain(&self.later).collect()
    }

    /// The latest revision, the one being reviewed.
    #[must_use]
    pub fn head(&self) -> &Revision {
        self.later.last().unwrap_or(&self.first)
    }

    /// The revision carrying that commit, if the proposal carries it at all.
    #[must_use]
    pub fn revision_of(&self, sha: &Sha) -> Option<&Revision> {
        self.revisions()
            .into_iter()
            .find(|revision| revision.sha() == sha)
    }

    /// The notes on the head revision that nobody has resolved. They are what
    /// stands between the proposal and the trunk.
    #[must_use]
    pub fn open_comments(&self) -> Vec<&Comment> {
        self.comments
            .iter()
            .filter(|comment| {
                comment.revision() == self.head().sha() && !self.is_resolved(comment.id())
            })
            .collect()
    }

    /// Every note nobody has resolved, from any revision.
    ///
    /// [`Proposal::open_comments`] is the subset that blocks; this is the
    /// subset you read, because a question that fell off the head when
    /// somebody committed is still a question nobody answered.
    #[must_use]
    pub fn conversation(&self) -> Vec<&Comment> {
        self.comments
            .iter()
            .filter(|comment| !self.is_resolved(comment.id()))
            .collect()
    }

    /// Every note, resolved or not, in log order.
    #[must_use]
    pub fn comments(&self) -> &[Comment] {
        &self.comments
    }

    /// Whether somebody said that note was answered.
    #[must_use]
    pub fn is_resolved(&self, id: &RecordId) -> bool {
        self.resolved.contains(id)
    }

    /// The replies to a note, oldest first, which is the thread.
    #[must_use]
    pub fn answers(&self, note: &RecordId) -> Vec<&Reply> {
        let mut thread: Vec<&Reply> = self
            .answers
            .iter()
            .filter(|reply| reply.target() == note)
            .collect();
        thread.sort_by_key(|reply| reply.at());
        thread
    }

    /// The results recorded against the head revision, by name.
    ///
    /// An older revision's result is not carried forward, because it ran on
    /// other code.
    #[must_use]
    pub fn checks(&self) -> Vec<&Check> {
        self.checks
            .values()
            .filter(|check| check.revision() == self.head().sha())
            .collect()
    }

    /// The checks on the head revision that said no.
    #[must_use]
    pub fn failing(&self) -> Vec<&Check> {
        self.checks()
            .into_iter()
            .filter(|check| !check.passed())
            .collect()
    }

    /// The one word a list column can carry about the checks.
    #[must_use]
    pub fn check_summary(&self) -> CheckSummary {
        if self.checks().is_empty() {
            return CheckSummary::None;
        }
        let failing = self.failing().len();
        if failing > 0 {
            CheckSummary::Failed(failing)
        } else {
            CheckSummary::Passing
        }
    }

    /// The decisions the author is explaining, in the order they were written,
    /// which is the order they should be read in.
    #[must_use]
    pub fn chunks(&self) -> &[Chunk] {
        &self.chunks
    }

    /// The author explaining the head revision.
    #[must_use]
    pub fn rationale(&self) -> Vec<&Rationale> {
        self.rationale
            .iter()
            .filter(|note| note.revision() == self.head().sha())
            .collect()
    }

    /// Every line an agent left on this proposal, on any revision.
    #[must_use]
    pub fn work(&self) -> &[Work] {
        &self.work
    }

    /// What the work log on the head revision adds up to.
    ///
    /// Head-scoped on purpose. A rebase that failed on revision one must not
    /// leave revision four reading as failed forever, which is what folding
    /// every revision's work into one answer did.
    #[must_use]
    pub fn activity(&self) -> Option<Activity> {
        let head: Vec<Work> = self
            .work
            .iter()
            .filter(|line| line.revision() == self.head().sha())
            .cloned()
            .collect();
        work::activity(&head)
    }

    /// Whether the head revision is waiting for an agent: somebody handed it
    /// over and nothing has picked it up since.
    #[must_use]
    pub fn dispatched(&self) -> bool {
        let head = self.head().sha();
        let Some(asked) = self
            .asks
            .iter()
            .filter(|ask| ask.revision() == head)
            .map(crate::work::Dispatch::at)
            .max()
        else {
            return false;
        };
        let since: Vec<Work> = self
            .work
            .iter()
            .filter(|line| line.revision() == head && line.at() >= asked)
            .cloned()
            .collect();
        // A claim handed back leaves the ask standing: a runner died and
        // nobody has actually answered yet. Anything else answered it,
        // including a failure, which waits for a person to ask again.
        work::activity(&since).is_none_or(|latest| latest.phase() == Phase::Cleared)
    }

    /// The one sentence both the terminal and the browser say about who is on
    /// this proposal.
    #[must_use]
    pub fn agent_line(&self) -> String {
        match self.activity() {
            Some(activity) if activity.working() => {
                format!(
                    "{} is {} since {}",
                    activity.agent(),
                    activity.task(),
                    clock(activity.since())
                )
            }
            Some(activity) if activity.failed() => {
                format!(
                    "{} failed: {}",
                    activity.task(),
                    activity.note().unwrap_or_default()
                )
            }
            Some(_) | None => {
                if self.dispatched() {
                    "waiting for an agent".to_owned()
                } else {
                    "no agent on it".to_owned()
                }
            }
        }
    }

    /// Why the proposal cannot land, and nothing when it can.
    ///
    /// The required checks are the ones the repository declares; a proposal
    /// that declares none is gated only by the review. A check that ran but
    /// nobody required does not block.
    ///
    /// # Errors
    ///
    /// A proposal that is not open, a head that still has notes on it, or a
    /// required check that is missing or failed.
    pub fn landable(&self, required: &[CheckName]) -> Result<()> {
        if self.state != State::Open {
            return Err(Error::NotOpen(self.state));
        }
        let open = self.open_comments().len();
        if open > 0 {
            return Err(Error::OpenComments {
                count: open,
                revision: self.head().number(),
            });
        }
        let ran = self.checks();
        for name in required {
            match ran.iter().find(|check| check.name() == name) {
                None => return Err(Error::CheckMissing(name.clone())),
                Some(check) if !check.passed() => return Err(Error::CheckFailed(name.clone())),
                Some(_) => {}
            }
        }
        Ok(())
    }
}

/// `HH:MM` out of a timestamp, which is what a chip has room for. The written
/// shape is fixed at `YYYY-MM-DDTHH:MM:SSZ`, so the hours start at eleven.
fn clock(at: Timestamp) -> String {
    at.to_string().get(11..16).unwrap_or_default().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::CheckStatus;
    use crate::fixtures::{check, dispatch, explanation, note, proposal, sha, work};
    use crate::record::Record;
    use crate::work::Task;

    // --- checks ---

    #[test]
    fn the_check_column_says_the_shortest_true_thing() -> Result<()> {
        let mut proposal = proposal();
        assert_eq!(proposal.check_summary(), CheckSummary::None);
        assert_eq!(proposal.check_summary().to_string(), "no checks");

        proposal.apply(Record::Check(check(&sha(1), "gate", CheckStatus::Passed)))?;
        assert_eq!(proposal.check_summary(), CheckSummary::Passing);
        assert_eq!(proposal.check_summary().to_string(), "passing");

        proposal.apply(Record::Check(check(&sha(1), "lint", CheckStatus::Failed)))?;
        assert_eq!(proposal.check_summary(), CheckSummary::Failed(1));
        assert_eq!(proposal.check_summary().to_string(), "1 failed");
        assert_eq!(proposal.failing().len(), 1);
        Ok(())
    }

    #[test]
    fn the_checks_come_back_in_the_same_order_every_time() -> Result<()> {
        let mut proposal = proposal();
        for name in ["suite", "gate", "lint"] {
            proposal.apply(Record::Check(check(&sha(1), name, CheckStatus::Passed)))?;
        }
        let names = |proposal: &Proposal| -> Vec<String> {
            proposal
                .checks()
                .iter()
                .map(|check| check.name().to_string())
                .collect()
        };
        assert_eq!(names(&proposal), vec!["gate", "lint", "suite"]);
        assert_eq!(names(&proposal), names(&proposal));
        Ok(())
    }

    #[test]
    fn the_last_result_under_a_name_is_the_one_that_counts() -> Result<()> {
        let mut proposal = proposal();
        proposal.apply(Record::Check(check(&sha(1), "gate", CheckStatus::Failed)))?;
        proposal.apply(Record::Check(check(&sha(1), "gate", CheckStatus::Passed)))?;
        assert_eq!(proposal.checks().len(), 1);
        assert_eq!(proposal.check_summary(), CheckSummary::Passing);
        Ok(())
    }

    // --- rationale ---

    #[test]
    fn an_explanation_left_on_an_older_revision_is_not_read_on_the_head() -> Result<()> {
        let mut proposal = proposal();
        proposal.apply(Record::Rationale(explanation(
            &sha(1),
            "the caller closes it",
        )))?;
        assert_eq!(proposal.rationale().len(), 1);
        proposal.add_revision(sha(2))?;
        assert_eq!(proposal.rationale().len(), 0);
        Ok(())
    }

    // --- activity ---

    #[test]
    fn a_started_record_makes_the_proposal_read_as_working() -> Result<()> {
        let mut proposal = proposal();
        proposal.apply(Record::Work(work(
            &sha(1),
            Task::Apply,
            Phase::Started,
            None,
            1_400,
        )))?;
        let activity = proposal.activity().ok_or(Error::NoAuthor)?;
        assert!(activity.working());
        assert_eq!(activity.task(), Task::Apply);
        assert_eq!(activity.agent().as_str(), "githerb-run");
        assert_eq!(proposal.agent_line(), "githerb-run is apply since 00:23");
        Ok(())
    }

    #[test]
    fn a_failure_says_what_failed_and_why() -> Result<()> {
        let mut proposal = proposal();
        proposal.apply(Record::Work(work(
            &sha(1),
            Task::Rebase,
            Phase::Failed,
            Some("conflicts in a.txt"),
            1_400,
        )))?;
        assert_eq!(proposal.agent_line(), "rebase failed: conflicts in a.txt");
        Ok(())
    }

    #[test]
    fn a_failure_on_an_older_revision_does_not_follow_the_proposal_forward() -> Result<()> {
        let mut proposal = proposal();
        proposal.apply(Record::Work(work(
            &sha(1),
            Task::Rebase,
            Phase::Failed,
            Some("conflicts in a.txt"),
            1_400,
        )))?;
        proposal.add_revision(sha(2))?;
        assert_eq!(proposal.activity(), None);
        assert_eq!(proposal.agent_line(), "no agent on it");
        assert_eq!(proposal.work().len(), 1);
        Ok(())
    }

    // --- dispatch ---

    #[test]
    fn a_dispatch_makes_the_proposal_read_as_waiting() -> Result<()> {
        let mut proposal = proposal();
        proposal.apply(Record::Dispatch(dispatch(&sha(1), 1_300)))?;
        assert!(proposal.dispatched());
        assert_eq!(proposal.agent_line(), "waiting for an agent");
        Ok(())
    }

    #[test]
    fn a_started_record_after_a_dispatch_answers_it() -> Result<()> {
        let mut proposal = proposal();
        proposal.apply(Record::Dispatch(dispatch(&sha(1), 1_300)))?;
        proposal.apply(Record::Work(work(
            &sha(1),
            Task::Apply,
            Phase::Started,
            None,
            1_400,
        )))?;
        assert!(!proposal.dispatched());
        Ok(())
    }

    #[test]
    fn a_claim_handed_back_leaves_the_dispatch_standing() -> Result<()> {
        let mut proposal = proposal();
        proposal.apply(Record::Dispatch(dispatch(&sha(1), 1_300)))?;
        proposal.apply(Record::Work(work(
            &sha(1),
            Task::Apply,
            Phase::Started,
            None,
            1_400,
        )))?;
        proposal.apply(Record::Work(work(
            &sha(1),
            Task::Apply,
            Phase::Cleared,
            Some("the runner that claimed this is gone"),
            1_500,
        )))?;
        let activity = proposal.activity().ok_or(Error::NoAuthor)?;
        assert!(activity.idle());
        assert!(proposal.dispatched());
        assert_eq!(proposal.agent_line(), "waiting for an agent");
        Ok(())
    }

    #[test]
    fn a_dispatch_on_an_older_revision_does_not_make_the_head_wait() -> Result<()> {
        let mut proposal = proposal();
        proposal.apply(Record::Dispatch(dispatch(&sha(1), 1_300)))?;
        proposal.add_revision(sha(2))?;
        assert!(!proposal.dispatched());
        assert_eq!(proposal.agent_line(), "no agent on it");
        Ok(())
    }

    #[test]
    fn nobody_is_on_a_proposal_nobody_touched() {
        assert_eq!(proposal().agent_line(), "no agent on it");
        assert_eq!(proposal().activity(), None);
    }

    // --- the notes themselves ---

    #[test]
    fn the_conversation_is_every_unresolved_note_in_log_order() -> Result<()> {
        let mut proposal = proposal();
        proposal.apply(Record::Comment(note(&sha(1), "first")))?;
        proposal.apply(Record::Comment(note(&sha(1), "second")))?;
        let bodies: Vec<&str> = proposal
            .conversation()
            .iter()
            .map(|comment| comment.body())
            .collect();
        assert_eq!(bodies, vec!["first", "second"]);
        assert_eq!(proposal.comments().len(), 2);
        Ok(())
    }
}
