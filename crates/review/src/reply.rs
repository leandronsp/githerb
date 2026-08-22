//! An answer to a note, filed under the note it answers.
//!
//! It carries no file and no lines of its own: it belongs to the thread of the
//! note, which is where it is read. A reply never blocks a landing. The
//! question is what blocks, and whoever asked it decides it was answered.

use crate::errors::{Error, Result};
use crate::identity::{Author, RecordId, Sha};
use crate::timestamp::Timestamp;
use crate::wire;

/// What somebody said under a note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    id: RecordId,
    target: RecordId,
    revision: Sha,
    body: String,
    author: Author,
    at: Timestamp,
}

impl Reply {
    /// The only way to build one.
    ///
    /// # Errors
    ///
    /// A body that says nothing.
    pub fn new(
        target: RecordId,
        revision: Sha,
        body: &str,
        author: Author,
        at: Timestamp,
    ) -> Result<Self> {
        let body = body.trim();
        if body.is_empty() {
            return Err(Error::NoBody);
        }
        let id = wire::reply_id(&target, &revision, body, &author, at);
        Ok(Self {
            id,
            target,
            revision,
            body: body.to_owned(),
            author,
            at,
        })
    }

    /// A reply read back from the log keeps the identity it was written with.
    pub(crate) fn restore(
        id: Option<RecordId>,
        target: RecordId,
        revision: Sha,
        body: &str,
        author: Author,
        at: Timestamp,
    ) -> Result<Self> {
        let mut reply = Self::new(target, revision, body, author, at)?;
        if let Some(id) = id {
            reply.id = id;
        }
        Ok(reply)
    }

    /// The identity derived from what it says.
    #[must_use]
    pub fn id(&self) -> &RecordId {
        &self.id
    }

    /// The note it answers.
    #[must_use]
    pub fn target(&self) -> &RecordId {
        &self.target
    }

    /// The head it was written against.
    #[must_use]
    pub fn revision(&self) -> &Sha {
        &self.revision
    }

    /// What it says.
    #[must_use]
    pub fn body(&self) -> &str {
        &self.body
    }

    /// Who said it, a person or an agent.
    #[must_use]
    pub fn author(&self) -> &Author {
        &self.author
    }

    /// When, to the second, in UTC.
    #[must_use]
    pub fn at(&self) -> Timestamp {
        self.at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &str = "9f6c1e2a3b4d5e6f708192a3b4c5d6e7f8091a2b";

    fn reply(body: &str) -> Result<Reply> {
        Reply::new(
            RecordId::parse("9b052da286a4")?,
            Sha::parse(HEAD)?,
            body,
            Author::parse("claude-code")?,
            Timestamp::from_unix(1_787_335_445),
        )
    }

    #[test]
    fn an_answer_belongs_to_the_note_it_answers() -> Result<()> {
        assert_eq!(reply("renamed it")?.target().as_str(), "9b052da286a4");
        Ok(())
    }

    #[test]
    fn an_answer_that_says_nothing_is_refused() {
        assert_eq!(reply("   "), Err(Error::NoBody));
    }

    #[test]
    fn an_answer_naming_no_note_is_refused() {
        assert_eq!(RecordId::parse(""), Err(Error::NoTarget));
    }
}
