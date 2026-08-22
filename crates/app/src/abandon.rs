//! Giving up on a proposal, which is a record like any other: the work stays
//! readable and the log says it never landed.

use review::{Author, Event, Proposal, ProposalId, Timestamp};

use crate::error::Result;
use crate::store::Store;

/// Say the proposal will not be landing.
///
/// # Errors
///
/// A proposal nobody opened, or one that already ended.
pub fn abandon(
    store: &Store,
    author: &Author,
    now: Timestamp,
    id: &ProposalId,
) -> Result<Proposal> {
    let mut proposal = store.load(id)?;

    proposal.abandon()?;

    store.record(
        &proposal,
        &Event::abandoned(id.clone(), author.clone(), now),
    )?;

    Ok(proposal)
}
