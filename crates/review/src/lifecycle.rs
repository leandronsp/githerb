//! The moments in a proposal's life: it opened, it landed, it was given up on,
//! or it now lands somewhere else.
//!
//! Everything between those moments is annotation. Like every other record
//! here an event is appended and never edited, so the state of a proposal is
//! what its events add up to rather than a field somebody rewrote.

use std::fmt;

use crate::branch::Branch;
use crate::errors::{Error, Result};
use crate::identity::{Author, ProposalId, Sha};
use crate::timestamp::Timestamp;

/// Which moment an event is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    /// The proposal started.
    Opened,
    /// It reached its target branch.
    Landed,
    /// It will not be landing.
    Abandoned,
    /// It lands somewhere else now.
    Retargeted,
}

impl EventKind {
    /// Read a kind off the wire.
    ///
    /// # Errors
    ///
    /// A kind this build does not know.
    pub fn parse(raw: &str) -> Result<Self> {
        match raw {
            "opened" => Ok(Self::Opened),
            "landed" => Ok(Self::Landed),
            "abandoned" => Ok(Self::Abandoned),
            "retargeted" => Ok(Self::Retargeted),
            _ => Err(Error::UnknownKind(raw.to_owned())),
        }
    }

    /// The word the wire format uses.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Opened => "opened",
            Self::Landed => "landed",
            Self::Abandoned => "abandoned",
            Self::Retargeted => "retargeted",
        }
    }
}

impl fmt::Display for EventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One line of the proposal log.
///
/// Build these through [`Event::opened`], [`Event::landed`],
/// [`Event::abandoned`] and [`Event::retargeted`]: those are where the rules
/// that the field types cannot carry are enforced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// The proposal started.
    Opened {
        /// Which proposal.
        id: ProposalId,
        /// What a person calls it. Never blank.
        title: String,
        /// The branch it means to land on.
        target: Branch,
        /// The commit it was cut from.
        base: Sha,
        /// Who opened it.
        author: Author,
        /// When.
        at: Timestamp,
    },
    /// It reached its target branch.
    Landed {
        /// Which proposal.
        id: ProposalId,
        /// Who landed it.
        author: Author,
        /// When.
        at: Timestamp,
    },
    /// It will not be landing.
    Abandoned {
        /// Which proposal.
        id: ProposalId,
        /// Who gave up on it.
        author: Author,
        /// When.
        at: Timestamp,
    },
    /// It lands somewhere else now, because the branch underneath it landed.
    Retargeted {
        /// Which proposal.
        id: ProposalId,
        /// The branch it lands on now.
        target: Branch,
        /// Who moved it.
        author: Author,
        /// When.
        at: Timestamp,
    },
}

impl Event {
    /// The event that starts a proposal.
    ///
    /// # Errors
    ///
    /// A blank title.
    pub fn opened(
        id: ProposalId,
        title: &str,
        target: Branch,
        base: Sha,
        author: Author,
        at: Timestamp,
    ) -> Result<Self> {
        let title = title.trim();
        if title.is_empty() {
            return Err(Error::NoTitle);
        }
        Ok(Self::Opened {
            id,
            title: title.to_owned(),
            target,
            base,
            author,
            at,
        })
    }

    /// The event that ends one.
    #[must_use]
    pub fn landed(id: ProposalId, author: Author, at: Timestamp) -> Self {
        Self::Landed { id, author, at }
    }

    /// The event for a proposal that will not be landing.
    #[must_use]
    pub fn abandoned(id: ProposalId, author: Author, at: Timestamp) -> Self {
        Self::Abandoned { id, author, at }
    }

    /// The event for a proposal that lands somewhere else now.
    ///
    /// It happens when the branch underneath it lands: a stack is a chain of
    /// proposals aimed at each other, and the one on top has to follow.
    #[must_use]
    pub fn retargeted(id: ProposalId, target: Branch, author: Author, at: Timestamp) -> Self {
        Self::Retargeted {
            id,
            target,
            author,
            at,
        }
    }

    /// Which moment this is.
    #[must_use]
    pub fn kind(&self) -> EventKind {
        match self {
            Self::Opened { .. } => EventKind::Opened,
            Self::Landed { .. } => EventKind::Landed,
            Self::Abandoned { .. } => EventKind::Abandoned,
            Self::Retargeted { .. } => EventKind::Retargeted,
        }
    }

    /// The proposal the event belongs to.
    #[must_use]
    pub fn id(&self) -> &ProposalId {
        match self {
            Self::Opened { id, .. }
            | Self::Landed { id, .. }
            | Self::Abandoned { id, .. }
            | Self::Retargeted { id, .. } => id,
        }
    }

    /// Who caused it.
    #[must_use]
    pub fn author(&self) -> &Author {
        match self {
            Self::Opened { author, .. }
            | Self::Landed { author, .. }
            | Self::Abandoned { author, .. }
            | Self::Retargeted { author, .. } => author,
        }
    }

    /// When, to the second, in UTC.
    #[must_use]
    pub fn at(&self) -> Timestamp {
        match self {
            Self::Opened { at, .. }
            | Self::Landed { at, .. }
            | Self::Abandoned { at, .. }
            | Self::Retargeted { at, .. } => *at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "00112233445566778899aabbccddeeff00112233";

    #[test]
    fn an_opened_event_carries_what_the_proposal_starts_as() -> Result<()> {
        let event = Event::opened(
            ProposalId::parse("gate")?,
            " Land the gate ",
            Branch::parse("main")?,
            Sha::parse(BASE)?,
            Author::parse("leandro")?,
            Timestamp::from_unix(1_787_335_445),
        )?;
        assert_eq!(event.kind(), EventKind::Opened);
        assert_eq!(event.id().as_str(), "gate");
        match event {
            Event::Opened { title, .. } => assert_eq!(title, "Land the gate"),
            Event::Landed { .. } | Event::Abandoned { .. } | Event::Retargeted { .. } => {
                panic!("expected an opened event")
            }
        }
        Ok(())
    }

    #[test]
    fn a_proposal_opened_with_no_title_is_refused() -> Result<()> {
        assert_eq!(
            Event::opened(
                ProposalId::parse("gate")?,
                "  ",
                Branch::parse("main")?,
                Sha::parse(BASE)?,
                Author::parse("leandro")?,
                Timestamp::from_unix(0),
            ),
            Err(Error::NoTitle)
        );
        Ok(())
    }

    #[test]
    fn every_event_kind_survives_a_round_trip_through_its_word() -> Result<()> {
        for kind in [
            EventKind::Opened,
            EventKind::Landed,
            EventKind::Abandoned,
            EventKind::Retargeted,
        ] {
            assert_eq!(EventKind::parse(kind.as_str())?, kind);
        }
        assert_eq!(
            EventKind::parse("summoned"),
            Err(Error::UnknownKind("summoned".to_owned()))
        );
        Ok(())
    }
}
