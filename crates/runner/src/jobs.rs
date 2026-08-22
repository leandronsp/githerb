//! What there is to do, derived from what the log says.
//!
//! Nothing here is stored. A job is read off a proposal every pass, so a
//! runner that dies mid-thought leaves no queue behind and the next one works
//! out the same answer from the same records.
//!
//! Two rules carry the whole module. At most one job per proposal, because two
//! agents on one branch is how work gets lost. And a failure on a revision is
//! never retried: a handover is the only thing that clears it, because a loop
//! that retries what already failed burns tokens all night.

use std::collections::HashSet;
use std::hash::BuildHasher;

use review::{Activity, CheckName, Proposal, ProposalId, State, Task};

/// A handover: the only thing that asks an agent to speak.
const HANDED_OVER: &str = "notes were handed over";

/// A handover with nothing left open, on a proposal the target ran past.
const HANDED_OVER_AND_BEHIND: &str = "handed over, and behind";

/// The mechanical case: git can replay this and no agent is involved.
const TARGET_RAN_AHEAD: &str = "the target ran ahead";

/// The head has never been through what the repository declares.
const NEVER_CHECKED: &str = "the head has not been checked";

/// One thing to do to one proposal.
///
/// `why` is a fixed sentence rather than free text: it is what the runner
/// prints when it claims the job, and a person reading two of them should be
/// able to tell the cases apart at a glance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    id: ProposalId,
    task: Task,
    why: &'static str,
}

impl Job {
    /// The only way to build one.
    #[must_use]
    pub fn new(id: ProposalId, task: Task, why: &'static str) -> Self {
        Self { id, task, why }
    }

    /// Which proposal it is about.
    #[must_use]
    pub fn id(&self) -> &ProposalId {
        &self.id
    }

    /// What to do to it.
    #[must_use]
    pub fn task(&self) -> Task {
        self.task
    }

    /// Why the log says so.
    #[must_use]
    pub fn why(&self) -> &'static str {
        self.why
    }
}

/// The work waiting across every proposal, at most one job each.
///
/// `stale` names the proposals whose target ran ahead of them, which is the
/// one thing the records cannot answer on their own: it is a fact about two
/// branches, not about anything anybody wrote down.
#[must_use]
pub fn pending<S: BuildHasher>(
    proposals: &[Proposal],
    stale: &HashSet<ProposalId, S>,
    required: &[CheckName],
) -> Vec<Job> {
    proposals
        .iter()
        .filter_map(|proposal| next(proposal, stale.contains(proposal.id()), required))
        .collect()
}

/// The one job a proposal is asking for, in the order the cases are decided.
fn next(proposal: &Proposal, stale: bool, required: &[CheckName]) -> Option<Job> {
    let activity = proposal.activity();

    if proposal.state() != State::Open || activity.as_ref().is_some_and(Activity::working) {
        return None;
    }

    let job = |task, why| Some(Job::new(proposal.id().clone(), task, why));

    // A handover is the trigger, and the only thing that clears a failure. An
    // agent that speaks without being asked is an agent nobody wants.
    if proposal.dispatched() {
        if stale && proposal.open_comments().is_empty() {
            return job(Task::Rebase, HANDED_OVER_AND_BEHIND);
        }

        return job(Task::Apply, HANDED_OVER);
    }

    if activity.as_ref().is_some_and(Activity::failed) {
        return None;
    }

    if stale {
        // Untriggered, so this one is mechanical: git replays it or nobody
        // touches it. No agent is called for a conflict nobody asked about.
        return job(Task::Rebase, TARGET_RAN_AHEAD);
    }

    if missing(proposal, required) {
        return job(Task::Check, NEVER_CHECKED);
    }

    None
}

/// Whether a name the repository requires has not run on the head.
fn missing(proposal: &Proposal, required: &[CheckName]) -> bool {
    let ran = proposal.checks();

    required
        .iter()
        .any(|name| !ran.iter().any(|check| check.name() == name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use review::{
        Anchor, Author, Branch, Check, CheckStatus, Comment, Dispatch, FilePath, Phase, Record,
        Sha, Side, Span, Timestamp, Work,
    };

    const BASE: &str = "00112233445566778899aabbccddeeff00112233";
    const HEAD: &str = "9f6c1e2a3b4d5e6f708192a3b4c5d6e7f8091a2b";

    // --- fixtures ---

    fn sha(hex: &str) -> Sha {
        Sha::parse(hex).unwrap()
    }

    fn at(minutes: i64) -> Timestamp {
        Timestamp::from_unix(minutes * 60)
    }

    fn proposal(records: Vec<Record>) -> Proposal {
        let mut proposal = Proposal::open(
            ProposalId::parse("p").unwrap(),
            "A proposal",
            Branch::parse("main").unwrap(),
            sha(BASE),
            sha(HEAD),
            at(0),
        )
        .unwrap();

        proposal.fold(records).unwrap();
        proposal
    }

    fn handed_over(minutes: i64) -> Record {
        Record::Dispatch(Dispatch::new(
            sha(HEAD),
            Author::parse("leandro").unwrap(),
            at(minutes),
        ))
    }

    fn noted(body: &str, minutes: i64) -> Record {
        Record::Comment(
            Comment::new(
                sha(HEAD),
                Anchor::new(
                    FilePath::parse("a.txt").unwrap(),
                    Span::new(Side::New, 2, 2).unwrap(),
                ),
                body,
                Author::parse("leandro").unwrap(),
                at(minutes),
            )
            .unwrap(),
        )
    }

    fn worked(task: Task, phase: Phase, minutes: i64) -> Record {
        Record::Work(
            Work::new(
                sha(HEAD),
                task,
                phase,
                Author::parse("githerb-run").unwrap(),
                None,
                at(minutes),
            )
            .unwrap(),
        )
    }

    fn checked(name: &str) -> Record {
        Record::Check(Check::new(
            CheckName::parse(name).unwrap(),
            CheckStatus::Passed,
            sha(HEAD),
            41,
            Author::parse("githerb-run").unwrap(),
            at(5),
        ))
    }

    fn gate() -> Vec<CheckName> {
        vec![CheckName::parse("gate").unwrap()]
    }

    fn behind(proposal: &Proposal) -> HashSet<ProposalId> {
        HashSet::from([proposal.id().clone()])
    }

    fn one_job(proposal: &Proposal, stale: bool) -> Option<Job> {
        let stale = if stale {
            behind(proposal)
        } else {
            HashSet::new()
        };

        pending(std::slice::from_ref(proposal), &stale, &gate())
            .into_iter()
            .next()
    }

    // --- what the pending work is ---

    #[test]
    fn handed_over_notes_are_applied() {
        let proposal = proposal(vec![handed_over(1)]);

        let job = one_job(&proposal, false).unwrap();

        assert_eq!(job.task(), Task::Apply);
        assert_eq!(job.why(), "notes were handed over");
        assert_eq!(job.id().as_str(), "p");
    }

    #[test]
    fn a_target_that_ran_ahead_is_rebased() {
        let proposal = proposal(vec![checked("gate")]);

        let job = one_job(&proposal, true).unwrap();

        assert_eq!(job.task(), Task::Rebase);
        assert_eq!(job.why(), "the target ran ahead");
    }

    #[test]
    fn a_head_nobody_checked_is_checked() {
        let proposal = proposal(vec![]);

        let job = one_job(&proposal, false).unwrap();

        assert_eq!(job.task(), Task::Check);
        assert_eq!(job.why(), "the head has not been checked");
    }

    #[test]
    fn handed_over_and_behind_with_notes_open_answers_the_notes_first() {
        let proposal = proposal(vec![noted("name this", 1), handed_over(2)]);

        let job = one_job(&proposal, true).unwrap();

        assert_eq!(job.task(), Task::Apply);
    }

    #[test]
    fn handed_over_and_behind_with_nothing_open_is_rebased() {
        let proposal = proposal(vec![handed_over(1)]);

        let job = one_job(&proposal, true).unwrap();

        assert_eq!(job.task(), Task::Rebase);
        assert_eq!(job.why(), "handed over, and behind");
    }

    #[test]
    fn a_proposal_somebody_already_picked_up_is_left_alone() {
        let proposal = proposal(vec![handed_over(1), worked(Task::Apply, Phase::Started, 2)]);

        assert_eq!(one_job(&proposal, true), None);
    }

    #[test]
    fn a_revision_that_was_given_up_on_is_never_retried() {
        let proposal = proposal(vec![worked(Task::Check, Phase::Failed, 2)]);

        assert_eq!(one_job(&proposal, true), None);
    }

    #[test]
    fn a_second_handover_clears_a_failure() {
        let proposal = proposal(vec![
            handed_over(1),
            worked(Task::Apply, Phase::Failed, 2),
            handed_over(3),
        ]);

        let job = one_job(&proposal, false).unwrap();

        assert_eq!(job.task(), Task::Apply);
    }

    // --- what pending does with more than one ---

    #[test]
    fn a_checked_head_with_nothing_open_asks_for_nothing() {
        let proposal = proposal(vec![checked("gate")]);

        assert_eq!(one_job(&proposal, false), None);
    }

    #[test]
    fn a_proposal_that_is_no_longer_open_asks_for_nothing() {
        let mut proposal = proposal(vec![handed_over(1)]);
        proposal.mark_landed(at(9));

        assert_eq!(one_job(&proposal, true), None);
    }

    #[test]
    fn every_proposal_gets_at_most_one_job() {
        let one = proposal(vec![handed_over(1)]);
        let two = proposal(vec![]);

        let jobs = pending(&[one, two], &HashSet::new(), &gate());

        assert_eq!(
            jobs.iter().map(Job::task).collect::<Vec<Task>>(),
            vec![Task::Apply, Task::Check]
        );
    }

    #[test]
    fn a_repository_that_requires_no_check_asks_for_nothing() {
        let proposal = proposal(vec![]);

        assert_eq!(pending(&[proposal], &HashSet::new(), &[]), vec![]);
    }
}
