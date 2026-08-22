//! Leaving a note on a range of lines. This is the human half of the loop.

use review::{Anchor, Author, Comment, ProposalId, Record, Timestamp};

use crate::error::Result;
use crate::store::Store;

/// Write a note against the head revision of a proposal.
///
/// The anchor arrives already parsed, because a file and a span are what the
/// core understands and a surface that has not turned its strings into one
/// has not finished reading its input. The aggregate is asked next, because
/// it is what knows whether the note belongs here at all: a line that would
/// not fold is a line that never reaches the log.
///
/// # Errors
///
/// A proposal nobody opened, one that is no longer open, or a body that says
/// nothing.
pub fn annotate(
    store: &Store,
    author: &Author,
    now: Timestamp,
    id: &ProposalId,
    anchor: Anchor,
    body: &str,
) -> Result<Comment> {
    let mut proposal = store.load(id)?;
    let head = proposal.head().sha().clone();

    let comment = Comment::new(head.clone(), anchor, body, author.clone(), now)?;
    let record = Record::Comment(comment.clone());

    proposal.apply(record.clone())?;
    store.annotate(&head, &record)?;

    Ok(comment)
}
