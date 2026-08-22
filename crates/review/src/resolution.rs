//! A note has been dealt with.
//!
//! It never edits the note, because the log is append-only: a resolution is a
//! new record that points at the record it resolves, which is what lets two
//! people annotate the same revision and git merge the result.

use crate::identity::{Author, RecordId};
use crate::timestamp::Timestamp;
use crate::wire;

/// Somebody saying a note is answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    id: RecordId,
    target: RecordId,
    author: Author,
    at: Timestamp,
}

impl Resolution {
    /// The only way to build one. Nothing here can be refused: the target and
    /// the author arrive already validated.
    #[must_use]
    pub fn new(target: RecordId, author: Author, at: Timestamp) -> Self {
        let id = wire::resolution_id(&target, &author, at);
        Self {
            id,
            target,
            author,
            at,
        }
    }

    /// A resolution read back from the log keeps the identity it was written with.
    #[must_use]
    pub(crate) fn restore(
        id: Option<RecordId>,
        target: RecordId,
        author: Author,
        at: Timestamp,
    ) -> Self {
        let mut resolution = Self::new(target, author, at);
        if let Some(id) = id {
            resolution.id = id;
        }
        resolution
    }

    /// The identity derived from what it points at.
    #[must_use]
    pub fn id(&self) -> &RecordId {
        &self.id
    }

    /// The note it resolves.
    #[must_use]
    pub fn target(&self) -> &RecordId {
        &self.target
    }

    /// Who resolved it.
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
    use crate::errors::Result;

    #[test]
    fn a_resolution_points_at_the_note_it_answers() -> Result<()> {
        let resolution = Resolution::new(
            RecordId::parse("9b052da286a4")?,
            Author::parse("claude")?,
            Timestamp::from_unix(1_787_335_445),
        );
        assert_eq!(resolution.target().as_str(), "9b052da286a4");
        assert_eq!(resolution.id().as_str().len(), 12);
        Ok(())
    }
}
