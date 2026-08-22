//! Landing: moving the target branch onto the head, and moving whatever was
//! stacked on it.
//!
//! It does not care which branch the target is. Landing onto another
//! proposal's branch is how a stack gets built before any of it reaches the
//! trunk, and because landing is fast-forward only the commits underneath a
//! stacked proposal never move: nothing is rebased, the base stays true, and
//! the one on top is simply told where it lands now.

use review::{Author, CheckName, Event, Proposal, ProposalId, Timestamp};

use crate::error::{Error, Result};
use crate::snapshot::Snapshot;
use crate::store::Store;

/// What happened: the proposal in its new state, and whatever had to follow.
#[derive(Debug, Clone)]
pub struct Landing {
    proposal: Proposal,
    followed: Vec<ProposalId>,
}

impl Landing {
    /// The proposal that landed.
    #[must_use]
    pub fn proposal(&self) -> &Proposal {
        &self.proposal
    }

    /// The proposals that were stacked on it and now land on its target.
    #[must_use]
    pub fn followed(&self) -> &[ProposalId] {
        &self.followed
    }
}

/// Land the proposal and move what was aimed at it.
///
/// # Errors
///
/// A proposal nobody opened, one that is not open, a head with notes still on
/// it, a required check that is missing or failed, or a target that moved on.
pub fn land(
    store: &Store,
    required: &[CheckName],
    author: &Author,
    now: Timestamp,
    id: &ProposalId,
) -> Result<Landing> {
    let snapshot = store.snapshot()?;
    let mut proposal = snapshot
        .get(id)
        .cloned()
        .ok_or_else(|| Error::NotFound(id.clone()))?;

    proposal.land(required)?;

    store.land(&proposal, &Event::landed(id.clone(), author.clone(), now))?;

    let followed = follow(store, &snapshot, &proposal, author, now)?;

    Ok(Landing { proposal, followed })
}

/// Move the proposals that were stacked on this one.
///
/// A proposal is stacked on it when the branch it lands on is sitting exactly
/// on this head, which after a fast-forward land is also where the target now
/// is. The snapshot the landing was read from is the one folded here: nothing
/// that moved since is a proposal this landing has to answer for.
fn follow(
    store: &Store,
    snapshot: &Snapshot,
    landed: &Proposal,
    author: &Author,
    now: Timestamp,
) -> Result<Vec<ProposalId>> {
    let head = landed.head().sha();
    let mut moved = Vec::new();

    for stacked in snapshot.open() {
        if stacked.target() == landed.target() {
            continue;
        }

        // A branch that is not there any more, or that is not sitting on this
        // head, is not stacked on it: not an answer to give, and not a
        // failure either.
        let Ok(tip) = store.repo().head_of(&stacked.target().git_ref()) else {
            continue;
        };

        if tip != head.as_str() {
            continue;
        }

        let event = Event::retargeted(
            stacked.id().clone(),
            landed.target().clone(),
            author.clone(),
            now,
        );
        store.record(stacked, &event)?;

        moved.push(stacked.id().clone());
    }

    Ok(moved)
}
