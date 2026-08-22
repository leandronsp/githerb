//! The aggregate root: a proposal, its revisions, and every rule that spans
//! more than one value.
//!
//! A note belongs to a revision of this proposal, a resolution answers a note
//! this proposal has seen, and landing is refused while the head still has
//! something open.
//!
//! Reading the log builds one of these by folding records into it, so this is
//! a mutable builder with immutable views rather than a value that copies nine
//! collections per record. What it never does is store what it can derive:
//! there is no status field, because a status field is a second copy of the
//! truth and it goes stale.

pub mod views;

use crate::branch::Branch;
use crate::check::{Check, CheckName};
use crate::chunk::Chunk;
use crate::comment::Comment;
use crate::errors::{Error, Result};
use crate::identity::{ProposalId, RecordId, Sha};
use crate::rationale::Rationale;
use crate::record::Record;
use crate::reply::Reply;
use crate::timestamp::Timestamp;
use crate::work::{Dispatch, Work};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Where a proposal is in its short life. There is no state for "in review",
/// because a proposal is always in review while it is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum State {
    /// Still being reviewed.
    Open,
    /// It reached its target branch.
    Landed,
    /// It will not be landing.
    Abandoned,
}

impl State {
    /// Read a state off the wire or a command line.
    ///
    /// # Errors
    ///
    /// A state this build does not know.
    pub fn parse(raw: &str) -> Result<Self> {
        match raw {
            "open" => Ok(Self::Open),
            "landed" => Ok(Self::Landed),
            "abandoned" => Ok(Self::Abandoned),
            _ => Err(Error::UnknownState(raw.to_owned())),
        }
    }

    /// The word the wire format uses.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Landed => "landed",
            Self::Abandoned => "abandoned",
        }
    }
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One attempt at a proposal: a commit, and where it sits in the sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revision {
    number: u32,
    sha: Sha,
}

impl Revision {
    /// Rebuild a revision that was already written down.
    #[must_use]
    pub fn new(number: u32, sha: Sha) -> Self {
        Self { number, sha }
    }

    /// Its place in the sequence, starting at one.
    #[must_use]
    pub fn number(&self) -> u32 {
        self.number
    }

    /// The commit it points at.
    #[must_use]
    pub fn sha(&self) -> &Sha {
        &self.sha
    }
}

/// A proposal and everything the log has said about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    id: ProposalId,
    title: String,
    target: Branch,
    base: Sha,
    state: State,
    opened_at: Timestamp,
    ended_at: Option<Timestamp>,
    /// Revision one, kept apart from the rest so that "a proposal always has a
    /// head" is a fact of the type rather than an invariant to remember.
    first: Revision,
    later: Vec<Revision>,
    comments: Vec<Comment>,
    resolved: BTreeSet<RecordId>,
    checks: BTreeMap<CheckName, Check>,
    chunks: Vec<Chunk>,
    rationale: Vec<Rationale>,
    work: Vec<Work>,
    asks: Vec<Dispatch>,
    answers: Vec<Reply>,
}

impl Proposal {
    /// Open a proposal at its first revision. The target is whichever branch
    /// this is meant to land on, which is often the trunk and need not be.
    ///
    /// # Errors
    ///
    /// A blank title, or a head that is the base: a proposal must move past
    /// where it was cut from.
    pub fn open(
        id: ProposalId,
        title: &str,
        target: Branch,
        base: Sha,
        head: Sha,
        opened_at: Timestamp,
    ) -> Result<Self> {
        let title = title.trim();
        if title.is_empty() {
            return Err(Error::NoTitle);
        }
        if base == head {
            return Err(Error::NothingProposed);
        }
        Ok(Self {
            id,
            title: title.to_owned(),
            target,
            base,
            state: State::Open,
            opened_at,
            ended_at: None,
            first: Revision::new(1, head),
            later: Vec::new(),
            comments: Vec::new(),
            resolved: BTreeSet::new(),
            checks: BTreeMap::new(),
            chunks: Vec::new(),
            rationale: Vec::new(),
            work: Vec::new(),
            asks: Vec::new(),
            answers: Vec::new(),
        })
    }

    /// Add the commit an agent produced after reading the annotations.
    ///
    /// # Errors
    ///
    /// A proposal that is no longer open, or a revision it already carries.
    pub fn add_revision(&mut self, sha: Sha) -> Result<()> {
        self.must_be_open()?;
        if self.revision_of(&sha).is_some() {
            return Err(Error::RevisionKnown(sha));
        }
        let number = self.head().number().saturating_add(1);
        self.later.push(Revision::new(number, sha));
        Ok(())
    }

    /// Fold one line of the log in. Applying the same record twice changes
    /// nothing, because the log may honestly deliver it twice.
    ///
    /// # Errors
    ///
    /// A proposal that is no longer open, a record about a revision it never
    /// saw, or an answer to a note it does not carry.
    pub fn apply(&mut self, record: Record) -> Result<()> {
        self.must_be_open()?;
        match record {
            Record::Comment(comment) => {
                self.must_know_revision(comment.revision())?;
                if !self.comments.iter().any(|seen| seen.id() == comment.id()) {
                    self.comments.push(comment);
                }
            }
            Record::Rationale(rationale) => {
                self.must_know_revision(rationale.revision())?;
                if !self
                    .rationale
                    .iter()
                    .any(|seen| seen.id() == rationale.id())
                {
                    self.rationale.push(rationale);
                }
            }
            Record::Reply(reply) => {
                self.must_know_comment(reply.target())?;
                if !self.answers.iter().any(|seen| seen.id() == reply.id()) {
                    self.answers.push(reply);
                }
            }
            Record::Resolve(resolution) => {
                self.must_know_comment(resolution.target())?;
                self.resolved.insert(resolution.target().clone());
            }
            Record::Check(check) => {
                self.must_know_revision(check.revision())?;
                self.checks.insert(check.name().clone(), check);
            }
            Record::Chunk(chunk) => self.chunks.push(chunk),
            Record::Work(work) => self.work.push(work),
            Record::Dispatch(dispatch) => self.asks.push(dispatch),
        }
        Ok(())
    }

    /// Fold a whole log in, answers last.
    ///
    /// A resolution and a reply name a note by its id, so the note has to be
    /// folded before they are, whatever order the log delivered them in.
    ///
    /// # Errors
    ///
    /// Whatever [`Proposal::apply`] refuses. The proposal keeps whatever
    /// folded before the refusal; the caller is expected to abandon it.
    pub fn fold(&mut self, records: impl IntoIterator<Item = Record>) -> Result<()> {
        let mut answers = Vec::new();
        for record in records {
            match record {
                Record::Reply(_) | Record::Resolve(_) => answers.push(record),
                Record::Comment(_)
                | Record::Rationale(_)
                | Record::Check(_)
                | Record::Chunk(_)
                | Record::Work(_)
                | Record::Dispatch(_) => self.apply(record)?,
            }
        }
        for record in answers {
            self.apply(record)?;
        }
        Ok(())
    }

    /// Point the proposal at another branch. Nothing else moves: the base is
    /// where the work was cut from and stays true whatever it lands on.
    ///
    /// # Errors
    ///
    /// A proposal that is no longer open.
    pub fn retarget(&mut self, target: Branch) -> Result<()> {
        self.must_be_open()?;
        self.target = target;
        Ok(())
    }

    /// Record that the head revision reached the target branch, because the
    /// log says it did.
    ///
    /// This runs no gate. Reading the log is not the moment to re-decide
    /// whether something that already landed was allowed to.
    pub fn mark_landed(&mut self, at: Timestamp) {
        self.state = State::Landed;
        self.ended_at = Some(at);
    }

    /// Record that the proposal was given up on, because the log says it was.
    pub fn mark_abandoned(&mut self, at: Timestamp) {
        self.state = State::Abandoned;
        self.ended_at = Some(at);
    }

    /// Run the gate and, if it opens, land the proposal.
    ///
    /// # Errors
    ///
    /// Whatever [`Proposal::landable`] refuses.
    pub fn land(&mut self, required: &[CheckName]) -> Result<()> {
        self.landable(required)?;
        self.state = State::Landed;
        Ok(())
    }

    /// Give up on a proposal, which is how something that did not get in stays
    /// visible instead of disappearing.
    ///
    /// # Errors
    ///
    /// A proposal that is no longer open.
    pub fn abandon(&mut self) -> Result<()> {
        self.must_be_open()?;
        self.state = State::Abandoned;
        Ok(())
    }

    fn must_be_open(&self) -> Result<()> {
        if self.state == State::Open {
            Ok(())
        } else {
            Err(Error::NotOpen(self.state))
        }
    }

    fn must_know_revision(&self, sha: &Sha) -> Result<()> {
        if self.revision_of(sha).is_some() {
            Ok(())
        } else {
            Err(Error::UnknownRevision(sha.clone()))
        }
    }

    fn must_know_comment(&self, id: &RecordId) -> Result<()> {
        if self.comments.iter().any(|comment| comment.id() == id) {
            Ok(())
        } else {
            Err(Error::UnknownComment(id.clone()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::CheckStatus;
    use crate::fixtures::{
        answer, author, check, dispatch, explanation, note, proposal, required, resolution, sha,
    };
    use crate::span::Anchor;
    use crate::work::{Phase, Task};

    // --- opening ---

    #[test]
    fn a_new_proposal_is_open_at_revision_one() {
        let proposal = proposal();
        assert_eq!(proposal.state(), State::Open);
        assert_eq!(proposal.head().number(), 1);
        assert_eq!(proposal.head().sha(), &sha(1));
        assert_eq!(proposal.target().as_str(), "main");
        assert_eq!(proposal.base(), &sha(0));
    }

    #[test]
    fn a_proposal_with_no_title_is_refused() {
        assert_eq!(
            Proposal::open(
                ProposalId::parse("gate").unwrap(),
                "  ",
                Branch::parse("main").unwrap(),
                sha(0),
                sha(1),
                Timestamp::from_unix(0),
            ),
            Err(Error::NoTitle)
        );
    }

    #[test]
    fn a_proposal_that_never_left_its_base_is_refused() {
        assert_eq!(
            Proposal::open(
                ProposalId::parse("gate").unwrap(),
                "Land the gate",
                Branch::parse("main").unwrap(),
                sha(1),
                sha(1),
                Timestamp::from_unix(0),
            ),
            Err(Error::NothingProposed)
        );
    }

    #[test]
    fn a_proposal_with_no_name_or_no_target_never_gets_built() {
        assert_eq!(ProposalId::parse(""), Err(Error::NoProposalId));
        assert_eq!(Branch::parse(""), Err(Error::NoBranch));
        assert_eq!(
            Sha::parse("nope"),
            Err(Error::NoRevision("nope".to_owned()))
        );
    }

    // --- revisions ---

    #[test]
    fn a_revision_is_appended_and_numbered() -> Result<()> {
        let mut proposal = proposal();
        proposal.add_revision(sha(2))?;
        assert_eq!(proposal.head().number(), 2);
        assert_eq!(proposal.revisions().len(), 2);
        assert_eq!(proposal.revision_of(&sha(1)).map(Revision::number), Some(1));
        Ok(())
    }

    #[test]
    fn a_revision_the_proposal_already_carries_is_refused() {
        let mut proposal = proposal();
        assert_eq!(
            proposal.add_revision(sha(1)),
            Err(Error::RevisionKnown(sha(1)))
        );
    }

    #[test]
    fn a_clone_of_a_proposal_does_not_move_when_the_original_does() -> Result<()> {
        let proposal = proposal();
        let mut next = proposal.clone();
        next.add_revision(sha(2))?;
        assert_eq!(proposal.head().number(), 1);
        assert_eq!(next.head().number(), 2);
        Ok(())
    }

    // --- folding ---

    #[test]
    fn a_note_about_a_revision_this_proposal_never_saw_is_refused() {
        let mut proposal = proposal();
        assert_eq!(
            proposal.apply(Record::Comment(note(&sha(9), "look here"))),
            Err(Error::UnknownRevision(sha(9)))
        );
    }

    #[test]
    fn a_resolution_naming_a_note_this_proposal_does_not_carry_is_refused() {
        let mut proposal = proposal();
        let stranger = note(&sha(1), "somewhere else");
        assert_eq!(
            proposal.apply(Record::Resolve(resolution(&stranger))),
            Err(Error::UnknownComment(stranger.id().clone()))
        );
    }

    #[test]
    fn the_same_note_folded_twice_counts_once() -> Result<()> {
        let mut proposal = proposal();
        let note = note(&sha(1), "this leaks");
        proposal.apply(Record::Comment(note.clone()))?;
        proposal.apply(Record::Comment(note))?;
        assert_eq!(proposal.comments().len(), 1);
        Ok(())
    }

    #[test]
    fn the_same_answer_folded_twice_counts_once() -> Result<()> {
        let mut proposal = proposal();
        let note = note(&sha(1), "this leaks");
        let answer = answer(&note, "renamed it", 1_400);
        proposal.fold([
            Record::Comment(note.clone()),
            Record::Reply(answer.clone()),
            Record::Reply(answer),
        ])?;
        assert_eq!(proposal.answers(note.id()).len(), 1);
        Ok(())
    }

    #[test]
    fn the_same_explanation_folded_twice_counts_once() -> Result<()> {
        let mut proposal = proposal();
        let explanation = explanation(&sha(1), "the handle is closed by the caller");
        proposal.apply(Record::Rationale(explanation.clone()))?;
        proposal.apply(Record::Rationale(explanation))?;
        assert_eq!(proposal.rationale().len(), 1);
        Ok(())
    }

    #[test]
    fn an_answer_that_arrives_before_the_note_it_answers_still_folds() -> Result<()> {
        let mut proposal = proposal();
        let note = note(&sha(1), "this leaks");
        proposal.fold([
            Record::Reply(answer(&note, "renamed it", 1_400)),
            Record::Resolve(resolution(&note)),
            Record::Comment(note.clone()),
        ])?;
        assert_eq!(proposal.answers(note.id()).len(), 1);
        assert!(proposal.is_resolved(note.id()));
        Ok(())
    }

    #[test]
    fn answers_come_back_oldest_first_however_they_were_folded() -> Result<()> {
        let mut proposal = proposal();
        let note = note(&sha(1), "this leaks");
        proposal.fold([
            Record::Comment(note.clone()),
            Record::Reply(answer(&note, "second", 1_500)),
            Record::Reply(answer(&note, "first", 1_400)),
        ])?;
        let bodies: Vec<&str> = proposal
            .answers(note.id())
            .iter()
            .map(|r| r.body())
            .collect();
        assert_eq!(bodies, vec!["first", "second"]);
        Ok(())
    }

    // --- the gate ---

    #[test]
    fn a_note_on_the_head_blocks_a_landing_until_it_is_resolved() -> Result<()> {
        let mut proposal = proposal();
        let note = note(&sha(1), "this leaks");
        proposal.apply(Record::Comment(note.clone()))?;
        assert_eq!(proposal.open_comments().len(), 1);
        assert_eq!(
            proposal.landable(&[]),
            Err(Error::OpenComments {
                count: 1,
                revision: 1
            })
        );
        assert_eq!(
            proposal.land(&[]),
            Err(Error::OpenComments {
                count: 1,
                revision: 1
            })
        );
        proposal.apply(Record::Resolve(resolution(&note)))?;
        assert_eq!(proposal.open_comments().len(), 0);
        assert_eq!(proposal.landable(&[]), Ok(()));
        Ok(())
    }

    #[test]
    fn a_note_left_on_an_older_revision_does_not_block_the_head() -> Result<()> {
        let mut proposal = proposal();
        proposal.apply(Record::Comment(note(&sha(1), "this leaks")))?;
        proposal.add_revision(sha(2))?;
        assert_eq!(proposal.open_comments().len(), 0);
        assert_eq!(proposal.conversation().len(), 1);
        assert_eq!(proposal.landable(&[]), Ok(()));
        Ok(())
    }

    #[test]
    fn an_answer_never_blocks_a_landing() -> Result<()> {
        let mut proposal = proposal();
        let note = note(&sha(1), "this leaks");
        proposal.fold([
            Record::Comment(note.clone()),
            Record::Reply(answer(&note, "renamed it", 1_400)),
            Record::Resolve(resolution(&note)),
        ])?;
        assert_eq!(proposal.landable(&[]), Ok(()));
        Ok(())
    }

    #[test]
    fn a_clean_proposal_lands_once_and_takes_nothing_afterwards() -> Result<()> {
        let mut proposal = proposal();
        proposal.land(&[])?;
        assert_eq!(proposal.state(), State::Landed);
        assert_eq!(proposal.land(&[]), Err(Error::NotOpen(State::Landed)));
        assert_eq!(
            proposal.apply(Record::Comment(note(&sha(1), "too late"))),
            Err(Error::NotOpen(State::Landed))
        );
        assert_eq!(
            proposal.add_revision(sha(2)),
            Err(Error::NotOpen(State::Landed))
        );
        Ok(())
    }

    #[test]
    fn a_required_check_that_never_ran_blocks_and_one_that_passed_opens_the_gate() -> Result<()> {
        let mut proposal = proposal();
        let gate = required("gate");
        assert_eq!(
            proposal.landable(std::slice::from_ref(&gate)),
            Err(Error::CheckMissing(gate.clone()))
        );
        proposal.apply(Record::Check(check(&sha(1), "gate", CheckStatus::Failed)))?;
        assert_eq!(
            proposal.landable(std::slice::from_ref(&gate)),
            Err(Error::CheckFailed(gate.clone()))
        );
        proposal.apply(Record::Check(check(&sha(1), "gate", CheckStatus::Passed)))?;
        assert_eq!(proposal.landable(std::slice::from_ref(&gate)), Ok(()));
        Ok(())
    }

    #[test]
    fn a_check_nobody_required_does_not_block() -> Result<()> {
        let mut proposal = proposal();
        proposal.apply(Record::Check(check(&sha(1), "lint", CheckStatus::Failed)))?;
        assert_eq!(proposal.landable(&[]), Ok(()));
        Ok(())
    }

    #[test]
    fn a_check_does_not_survive_a_new_revision() -> Result<()> {
        let mut proposal = proposal();
        let gate = required("gate");
        proposal.apply(Record::Check(check(&sha(1), "gate", CheckStatus::Passed)))?;
        proposal.add_revision(sha(2))?;
        assert_eq!(proposal.checks().len(), 0);
        assert_eq!(
            proposal.landable(std::slice::from_ref(&gate)),
            Err(Error::CheckMissing(gate))
        );
        Ok(())
    }

    // --- endings ---

    #[test]
    fn a_proposal_can_be_given_up_on_once() -> Result<()> {
        let mut proposal = proposal();
        proposal.abandon()?;
        assert_eq!(proposal.state(), State::Abandoned);
        assert_eq!(proposal.abandon(), Err(Error::NotOpen(State::Abandoned)));
        assert_eq!(proposal.land(&[]), Err(Error::NotOpen(State::Abandoned)));
        Ok(())
    }

    #[test]
    fn reading_the_log_does_not_re_run_the_gate() -> Result<()> {
        let mut proposal = proposal();
        proposal.apply(Record::Comment(note(&sha(1), "this leaks")))?;
        proposal.mark_landed(Timestamp::from_unix(2_000));
        assert_eq!(proposal.state(), State::Landed);
        assert_eq!(proposal.ended_at(), Some(Timestamp::from_unix(2_000)));
        Ok(())
    }

    #[test]
    fn a_proposal_that_was_given_up_on_reads_back_as_abandoned() {
        let mut proposal = proposal();
        proposal.mark_abandoned(Timestamp::from_unix(2_000));
        assert_eq!(proposal.state(), State::Abandoned);
    }

    #[test]
    fn retargeting_moves_where_it_lands_and_never_the_base() -> Result<()> {
        let mut proposal = proposal();
        proposal.retarget(Branch::parse("release-2.1")?)?;
        assert_eq!(proposal.target().as_str(), "release-2.1");
        assert_eq!(proposal.base(), &sha(0));
        proposal.land(&[])?;
        assert_eq!(
            proposal.retarget(Branch::parse("main")?),
            Err(Error::NotOpen(State::Landed))
        );
        Ok(())
    }

    // --- state ---

    #[test]
    fn a_state_this_build_does_not_know_is_refused() -> Result<()> {
        for state in [State::Open, State::Landed, State::Abandoned] {
            assert_eq!(State::parse(state.as_str())?, state);
        }
        assert_eq!(
            State::parse("in review"),
            Err(Error::UnknownState("in review".to_owned()))
        );
        Ok(())
    }

    // --- what a chunk and a dispatch do not need ---

    #[test]
    fn chunks_dispatches_and_work_lines_are_kept_in_the_order_they_arrived() -> Result<()> {
        let mut proposal = proposal();
        proposal.fold([
            Record::Chunk(crate::chunk::Chunk::new("t", None, "b", "a", "d", None)?),
            Record::Work(crate::fixtures::work(
                &sha(1),
                Task::Apply,
                Phase::Started,
                None,
                1_400,
            )),
            Record::Dispatch(dispatch(&sha(1), 1_350)),
        ])?;
        assert_eq!(proposal.chunks().len(), 1);
        assert_eq!(proposal.work().len(), 1);
        assert!(proposal.dispatched() || proposal.activity().is_some());
        Ok(())
    }

    #[test]
    fn a_chunk_anchored_on_the_wire_keeps_its_anchor_through_the_fold() -> Result<()> {
        let mut proposal = proposal();
        let chunk =
            crate::chunk::Chunk::new("t", None, "b", "a", "d", None)?.anchored(Anchor::new(
                crate::identity::FilePath::parse("a.go")?,
                crate::span::Span::new(crate::span::Side::New, 1, 2)?,
            ));
        proposal.apply(Record::Chunk(chunk))?;
        assert!(
            proposal
                .chunks()
                .first()
                .is_some_and(|c| c.anchor().is_some())
        );
        Ok(())
    }

    #[test]
    fn an_author_is_needed_before_a_record_exists() {
        assert_eq!(author("leandro").as_str(), "leandro");
    }
}
