//! Handing the open notes to an agent: a person saying "somebody answer
//! these", which the runner reads as a job.

use review::{Author, Dispatch, Proposal, ProposalId, Record, Timestamp};

use crate::error::Result;
use crate::store::Store;

/// Ask for an agent on the head revision.
///
/// # Errors
///
/// A proposal nobody opened, or one that is no longer open.
pub fn dispatch(
    store: &Store,
    author: &Author,
    now: Timestamp,
    id: &ProposalId,
) -> Result<Proposal> {
    let mut proposal = store.load(id)?;
    let head = proposal.head().sha().clone();
    let record = Record::Dispatch(Dispatch::new(head.clone(), author.clone(), now));

    proposal.apply(record.clone())?;
    store.annotate(&head, &record)?;

    Ok(proposal)
}
