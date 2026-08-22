//! The author explaining some lines, rather than asking about them.
//!
//! It has the shape of a note and none of its force: rationale never blocks a
//! landing, because it answers a question instead of asking one. It is its own
//! kind on the wire, so its identity is derived over `"kind":"rationale"` and
//! a note with the same words is a different record.

use crate::errors::{Error, Result};
use crate::identity::{Author, RecordId, Sha};
use crate::record::Kind;
use crate::span::Anchor;
use crate::timestamp::Timestamp;
use crate::wire;

/// Why some lines are the way they are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rationale {
    id: RecordId,
    revision: Sha,
    anchor: Anchor,
    body: String,
    author: Author,
    at: Timestamp,
}

impl Rationale {
    /// The only way to build one.
    ///
    /// # Errors
    ///
    /// A body that says nothing.
    pub fn new(
        revision: Sha,
        anchor: Anchor,
        body: &str,
        author: Author,
        at: Timestamp,
    ) -> Result<Self> {
        let body = body.trim();
        if body.is_empty() {
            return Err(Error::NoBody);
        }
        let id = wire::note_id(Kind::Rationale, &revision, &anchor, body, &author, at);
        Ok(Self {
            id,
            revision,
            anchor,
            body: body.to_owned(),
            author,
            at,
        })
    }

    /// A rationale read back from the log keeps the identity it was written
    /// with, whatever this build would derive for it today.
    pub(crate) fn restore(
        id: Option<RecordId>,
        revision: Sha,
        anchor: Anchor,
        body: &str,
        author: Author,
        at: Timestamp,
    ) -> Result<Self> {
        let mut note = Self::new(revision, anchor, body, author, at)?;
        if let Some(id) = id {
            note.id = id;
        }
        Ok(note)
    }

    /// The identity derived from what it says.
    #[must_use]
    pub fn id(&self) -> &RecordId {
        &self.id
    }

    /// The commit it explains.
    #[must_use]
    pub fn revision(&self) -> &Sha {
        &self.revision
    }

    /// The file and the lines it explains.
    #[must_use]
    pub fn anchor(&self) -> &Anchor {
        &self.anchor
    }

    /// The explanation.
    #[must_use]
    pub fn body(&self) -> &str {
        &self.body
    }

    /// Who wrote it.
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
    use crate::comment::Comment;
    use crate::identity::FilePath;
    use crate::span::{Side, Span};

    const HEAD: &str = "9f6c1e2a3b4d5e6f708192a3b4c5d6e7f8091a2b";

    fn parts() -> Result<(Sha, Anchor, Author, Timestamp)> {
        Ok((
            Sha::parse(HEAD)?,
            Anchor::new(
                FilePath::parse("internal/app/land.go")?,
                Span::new(Side::New, 42, 47)?,
            ),
            Author::parse("leandro")?,
            Timestamp::from_unix(1_787_335_445),
        ))
    }

    #[test]
    fn a_rationale_is_not_the_note_that_says_the_same_words() -> Result<()> {
        let (revision, anchor, author, at) = parts()?;
        let explained = Rationale::new(
            revision.clone(),
            anchor.clone(),
            "because",
            author.clone(),
            at,
        )?;
        let asked = Comment::new(revision, anchor, "because", author, at)?;
        assert_ne!(explained.id(), asked.id());
        Ok(())
    }

    #[test]
    fn a_rationale_that_says_nothing_is_refused() -> Result<()> {
        let (revision, anchor, author, at) = parts()?;
        assert_eq!(
            Rationale::new(revision, anchor, " ", author, at),
            Err(Error::NoBody)
        );
        Ok(())
    }
}
