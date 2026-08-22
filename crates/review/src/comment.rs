//! A note on a range of lines of one revision: the thing that blocks a landing.
//!
//! Its identity is derived from what it says, so the same note written twice
//! is one note and an append-only log deduplicates itself.

use crate::errors::{Error, Result};
use crate::identity::{Author, RecordId, Sha};
use crate::record::Kind;
use crate::span::Anchor;
use crate::timestamp::Timestamp;
use crate::wire;

/// A person's note on some lines, asking for something.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    id: RecordId,
    revision: Sha,
    anchor: Anchor,
    body: String,
    author: Author,
    at: Timestamp,
}

impl Comment {
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
        let id = wire::note_id(Kind::Comment, &revision, &anchor, body, &author, at);
        Ok(Self {
            id,
            revision,
            anchor,
            body: body.to_owned(),
            author,
            at,
        })
    }

    /// A note read back from the log keeps the identity it was written with,
    /// whatever this build would derive for it today.
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

    /// The identity derived from what the note says.
    #[must_use]
    pub fn id(&self) -> &RecordId {
        &self.id
    }

    /// The commit the note applies to.
    #[must_use]
    pub fn revision(&self) -> &Sha {
        &self.revision
    }

    /// The file and the lines it points at.
    #[must_use]
    pub fn anchor(&self) -> &Anchor {
        &self.anchor
    }

    /// What the note says.
    #[must_use]
    pub fn body(&self) -> &str {
        &self.body
    }

    /// Who left it.
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
    use crate::identity::FilePath;
    use crate::span::{Side, Span};

    const HEAD: &str = "9f6c1e2a3b4d5e6f708192a3b4c5d6e7f8091a2b";

    fn anchor() -> Result<Anchor> {
        Ok(Anchor::new(
            FilePath::parse("internal/app/land.go")?,
            Span::new(Side::New, 42, 47)?,
        ))
    }

    fn comment(body: &str) -> Result<Comment> {
        Comment::new(
            Sha::parse(HEAD)?,
            anchor()?,
            body,
            Author::parse("leandro")?,
            Timestamp::from_unix(1_787_335_445),
        )
    }

    #[test]
    fn the_same_note_written_twice_has_the_same_identity() -> Result<()> {
        assert_eq!(comment("this leaks")?.id(), comment("this leaks")?.id());
        Ok(())
    }

    #[test]
    fn a_different_note_has_a_different_identity() -> Result<()> {
        assert_ne!(comment("this leaks")?.id(), comment("this does not")?.id());
        Ok(())
    }

    #[test]
    fn a_note_that_says_nothing_is_refused() {
        assert_eq!(comment("  \t "), Err(Error::NoBody));
    }

    #[test]
    fn a_body_is_trimmed_before_it_is_hashed() -> Result<()> {
        assert_eq!(comment("  this leaks  ")?.id(), comment("this leaks")?.id());
        assert_eq!(comment("  this leaks  ")?.body(), "this leaks");
        Ok(())
    }
}
