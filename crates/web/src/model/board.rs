//! The board: every proposal in the repository, grouped by what became of it.
//!
//! The sizes arrive from the caller rather than being measured here, because
//! measuring one means a diff per proposal and that is what made the old board
//! cost twenty git processes to draw a list.

use std::collections::HashMap;

use review::{Proposal, ProposalId, State};

/// One proposal as a list reads it.
#[derive(Debug, Clone)]
pub struct Entry {
    id: String,
    title: String,
    target: String,
    revision: u32,
    notes: usize,
    added: usize,
    removed: usize,
    checks: String,
    at: String,
    /// When it last moved, which is what the groups sort on.
    sort: i64,
}

impl Entry {
    /// What the proposal is called on the wire.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// What it is called in prose.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The branch it wants to land on.
    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Which revision is at the head.
    #[must_use]
    pub fn revision(&self) -> u32 {
        self.revision
    }

    /// How many notes on the head nobody has resolved.
    #[must_use]
    pub fn notes(&self) -> usize {
        self.notes
    }

    /// Lines added.
    #[must_use]
    pub fn added(&self) -> usize {
        self.added
    }

    /// Lines removed.
    #[must_use]
    pub fn removed(&self) -> usize {
        self.removed
    }

    /// The one word the checks add up to.
    #[must_use]
    pub fn checks(&self) -> &str {
        &self.checks
    }

    /// When it was opened, as a date.
    #[must_use]
    pub fn at(&self) -> &str {
        &self.at
    }
}

/// Every proposal, in three groups.
#[derive(Debug, Clone, Default)]
pub struct Board {
    open: Vec<Entry>,
    landed: Vec<Entry>,
    abandoned: Vec<Entry>,
}

impl Board {
    /// Group the proposals by state, newest first inside each group.
    ///
    /// `sizes` carries `(added, removed)` per proposal; a proposal the caller
    /// did not measure shows no counts rather than costing a diff here.
    #[must_use]
    pub fn build(proposals: &[Proposal], sizes: &HashMap<ProposalId, (usize, usize)>) -> Self {
        let mut board = Self::default();
        for proposal in proposals {
            let (added, removed) = sizes.get(proposal.id()).copied().unwrap_or((0, 0));
            let entry = Entry {
                id: proposal.id().as_str().to_owned(),
                title: proposal.title().to_owned(),
                target: proposal.target().as_str().to_owned(),
                revision: proposal.head().number(),
                notes: proposal.open_comments().len(),
                added,
                removed,
                checks: proposal.check_summary().to_string(),
                at: day(proposal),
                sort: proposal
                    .ended_at()
                    .unwrap_or_else(|| proposal.opened_at())
                    .unix(),
            };
            match proposal.state() {
                State::Open => board.open.push(entry),
                State::Landed => board.landed.push(entry),
                State::Abandoned => board.abandoned.push(entry),
            }
        }
        for group in [&mut board.open, &mut board.landed, &mut board.abandoned] {
            group.sort_by_key(|entry| std::cmp::Reverse(entry.sort));
        }
        board
    }

    /// The proposals still in review.
    #[must_use]
    pub fn open(&self) -> &[Entry] {
        &self.open
    }

    /// The proposals that got in.
    #[must_use]
    pub fn landed(&self) -> &[Entry] {
        &self.landed
    }

    /// The proposals that did not.
    #[must_use]
    pub fn abandoned(&self) -> &[Entry] {
        &self.abandoned
    }

    /// How many proposals the board carries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.open.len() + self.landed.len() + self.abandoned.len()
    }

    /// Whether the repository has no proposals at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The day a proposal was opened, which is all a list has room for.
fn day(proposal: &Proposal) -> String {
    let at = proposal.opened_at().to_string();
    at.get(0..10).unwrap_or_default().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{at, proposal, sha};
    use review::{Branch, ProposalId};

    /// A proposal of that name, opened that many minutes in.
    fn named(id: &str, minutes: i64) -> Proposal {
        Proposal::open(
            ProposalId::parse(id).unwrap(),
            "A slice",
            Branch::parse("main").unwrap(),
            sha('a'),
            sha('b'),
            at(minutes),
        )
        .unwrap()
    }

    #[test]
    fn proposals_are_grouped_by_what_became_of_them() {
        let mut landed = named("older", 10);
        landed.mark_landed(at(30));
        let mut gone = named("dropped", 20);
        gone.mark_abandoned(at(40));
        let board = Board::build(&[named("newer", 50), landed, gone], &HashMap::new());
        assert_eq!(board.open().len(), 1);
        assert_eq!(board.landed()[0].id(), "older");
        assert_eq!(board.abandoned()[0].id(), "dropped");
        assert_eq!(board.len(), 3);
    }

    #[test]
    fn the_newest_proposal_is_first() {
        let board = Board::build(
            &[named("first", 10), named("third", 90), named("second", 50)],
            &HashMap::new(),
        );
        let order: Vec<&str> = board.open().iter().map(Entry::id).collect();
        assert_eq!(order, vec!["third", "second", "first"]);
    }

    #[test]
    fn the_sizes_come_from_the_caller_and_never_from_a_diff() {
        let proposal = proposal();
        let mut sizes = HashMap::new();
        sizes.insert(proposal.id().clone(), (12, 4));
        let board = Board::build(&[proposal], &sizes);
        assert_eq!(
            (board.open()[0].added(), board.open()[0].removed()),
            (12, 4)
        );
    }

    #[test]
    fn a_proposal_nobody_measured_shows_no_counts() {
        let board = Board::build(&[proposal()], &HashMap::new());
        assert_eq!((board.open()[0].added(), board.open()[0].removed()), (0, 0));
        assert_eq!(board.open()[0].checks(), "no checks");
    }

    #[test]
    fn an_empty_repository_has_an_empty_board() {
        let board = Board::build(&[], &HashMap::new());
        assert!(board.is_empty(), "nothing proposed");
    }
}
