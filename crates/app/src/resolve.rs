//! Saying a note is dealt with, which is the only thing that takes it out of
//! the way of the trunk.

use review::{Author, ProposalId, Record, RecordId, Resolution, Timestamp};

use crate::error::Result;
use crate::store::Store;

/// Resolve a note on a proposal.
///
/// # Errors
///
/// A proposal nobody opened, or a note this proposal does not carry.
pub fn resolve(
    store: &Store,
    author: &Author,
    now: Timestamp,
    id: &ProposalId,
    note: &RecordId,
) -> Result<()> {
    let mut proposal = store.load(id)?;
    let record = Record::Resolve(Resolution::new(note.clone(), author.clone(), now));

    proposal.apply(record.clone())?;

    store.annotate(proposal.head().sha(), &record)
}
