//! An answer under a note. It says something and it blocks nothing: only a
//! resolution takes a note out of the way.

use review::{Author, ProposalId, Record, RecordId, Reply, Timestamp};

use crate::error::Result;
use crate::store::Store;

/// Answer a note, in words.
///
/// # Errors
///
/// A proposal nobody opened, a note this proposal does not carry, or an
/// answer that says nothing.
pub fn reply(
    store: &Store,
    author: &Author,
    now: Timestamp,
    id: &ProposalId,
    note: &RecordId,
    body: &str,
) -> Result<Reply> {
    let mut proposal = store.load(id)?;
    let head = proposal.head().sha().clone();

    let answer = Reply::new(note.clone(), head.clone(), body, author.clone(), now)?;
    let record = Record::Reply(answer.clone());

    // A reply names a note by id, and a line naming a note nobody has is a
    // line the proposal cannot fold. Refusing it here keeps that out of the
    // log rather than leaving it to poison every later read.
    proposal.apply(record.clone())?;
    store.annotate(&head, &record)?;

    Ok(answer)
}
