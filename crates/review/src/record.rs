//! One line of the annotation log, whatever it happens to say.
//!
//! Everything a person or an agent appends to a revision is a `Record`. A kind
//! this build does not know is skipped by the store rather than fatal, which is
//! what lets a newer binary write a new kind without breaking every older one.

use std::fmt;

use crate::check::Check;
use crate::chunk::Chunk;
use crate::comment::Comment;
use crate::errors::{Error, Result};
use crate::identity::RecordId;
use crate::rationale::Rationale;
use crate::reply::Reply;
use crate::resolution::Resolution;
use crate::work::{Dispatch, Work};

/// What a record is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A note asking for something.
    Comment,
    /// A note saying a note is answered.
    Resolve,
    /// A result from a command that ran on a revision.
    Check,
    /// A reviewable decision the author is explaining.
    Chunk,
    /// The author explaining some lines.
    Rationale,
    /// A line of what an agent did.
    Work,
    /// A person handing the open notes to an agent.
    Dispatch,
    /// An answer under a note.
    Reply,
}

impl Kind {
    /// Read a kind off the wire.
    ///
    /// # Errors
    ///
    /// A kind this build does not know. The store skips these.
    pub fn parse(raw: &str) -> Result<Self> {
        match raw {
            "comment" => Ok(Self::Comment),
            "resolve" => Ok(Self::Resolve),
            "check" => Ok(Self::Check),
            "chunk" => Ok(Self::Chunk),
            "rationale" => Ok(Self::Rationale),
            "work" => Ok(Self::Work),
            "dispatch" => Ok(Self::Dispatch),
            "reply" => Ok(Self::Reply),
            _ => Err(Error::UnknownKind(raw.to_owned())),
        }
    }

    /// The word the wire format uses.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Comment => "comment",
            Self::Resolve => "resolve",
            Self::Check => "check",
            Self::Chunk => "chunk",
            Self::Rationale => "rationale",
            Self::Work => "work",
            Self::Dispatch => "dispatch",
            Self::Reply => "reply",
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One line of the annotation log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Record {
    /// A note asking for something.
    Comment(Comment),
    /// The author explaining some lines.
    Rationale(Rationale),
    /// An answer under a note.
    Reply(Reply),
    /// A note saying a note is answered.
    Resolve(Resolution),
    /// A result from a command that ran on a revision.
    Check(Check),
    /// A reviewable decision.
    Chunk(Chunk),
    /// A line of what an agent did.
    Work(Work),
    /// A person handing the open notes to an agent.
    Dispatch(Dispatch),
}

impl Record {
    /// What kind of record this is.
    #[must_use]
    pub fn kind(&self) -> Kind {
        match self {
            Self::Comment(_) => Kind::Comment,
            Self::Rationale(_) => Kind::Rationale,
            Self::Reply(_) => Kind::Reply,
            Self::Resolve(_) => Kind::Resolve,
            Self::Check(_) => Kind::Check,
            Self::Chunk(_) => Kind::Chunk,
            Self::Work(_) => Kind::Work,
            Self::Dispatch(_) => Kind::Dispatch,
        }
    }

    /// The identity of the record, for the four kinds that have one. A check,
    /// a chunk, a work line and a dispatch are anonymous: nothing points at
    /// them, so nothing needs to name them.
    #[must_use]
    pub fn id(&self) -> Option<&RecordId> {
        match self {
            Self::Comment(comment) => Some(comment.id()),
            Self::Rationale(rationale) => Some(rationale.id()),
            Self::Reply(reply) => Some(reply.id()),
            Self::Resolve(resolution) => Some(resolution.id()),
            Self::Check(_) | Self::Chunk(_) | Self::Work(_) | Self::Dispatch(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_survives_a_round_trip_through_its_word() -> Result<()> {
        for kind in [
            Kind::Comment,
            Kind::Resolve,
            Kind::Check,
            Kind::Chunk,
            Kind::Rationale,
            Kind::Work,
            Kind::Dispatch,
            Kind::Reply,
        ] {
            assert_eq!(Kind::parse(kind.as_str())?, kind);
        }
        Ok(())
    }

    #[test]
    fn a_kind_from_the_future_is_refused_by_name() {
        assert_eq!(
            Kind::parse("telepathy"),
            Err(Error::UnknownKind("telepathy".to_owned()))
        );
    }
}
