//! An event to its line, and a line back to an event.
//!
//! The proposal log is a smaller shape than the annotation log: an event only
//! ever says which proposal, who, when, and for an opening what it is called
//! and where it came from.

use serde::{Deserialize, Serialize};

use super::line::VERSION;
use crate::branch::Branch;
use crate::errors::{Error, Result};
use crate::identity::{Author, ProposalId, Sha};
use crate::lifecycle::{Event, EventKind};
use crate::timestamp::Timestamp;

/// The wire shape of a proposal event. Field order is the order on disk.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct EventLine {
    v: i64,
    kind: String,
    id: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    title: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    target: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    base: String,
    author: String,
    at: String,
}

impl Event {
    /// Render the event as the single line it is stored as, with no newline.
    #[must_use]
    pub fn to_line(&self) -> String {
        let common = EventLine {
            v: VERSION,
            kind: self.kind().as_str().to_owned(),
            id: self.id().to_string(),
            author: self.author().to_string(),
            at: self.at().to_string(),
            ..EventLine::default()
        };
        let line = match self {
            Self::Opened {
                title,
                target,
                base,
                ..
            } => EventLine {
                title: title.clone(),
                target: target.to_string(),
                base: base.to_string(),
                ..common
            },
            Self::Retargeted { target, .. } => EventLine {
                target: target.to_string(),
                ..common
            },
            Self::Landed { .. } | Self::Abandoned { .. } => common,
        };
        serde_json::to_string(&line).unwrap_or_default()
    }

    /// Read one line of the proposal log.
    ///
    /// # Errors
    ///
    /// Text that is not JSON, a version this build does not speak, a kind it
    /// does not know, or an event any constructor here refuses.
    pub fn parse_line(raw: &str) -> Result<Self> {
        let line: EventLine =
            serde_json::from_str(raw).map_err(|_| Error::Malformed(raw.to_owned()))?;
        if line.v != VERSION {
            return Err(Error::Version(line.v));
        }
        let at = Timestamp::parse(&line.at)
            .map_err(|_| Error::Malformed(format!("timestamp {}", line.at)))?;
        let id = ProposalId::parse(&line.id)?;
        let author = Author::parse(&line.author)?;

        match EventKind::parse(&line.kind)? {
            EventKind::Opened => Self::opened(
                id,
                &line.title,
                Branch::parse(&line.target)?,
                Sha::parse(&line.base)?,
                author,
                at,
            ),
            EventKind::Landed => Ok(Self::landed(id, author, at)),
            EventKind::Abandoned => Ok(Self::abandoned(id, author, at)),
            EventKind::Retargeted => Ok(Self::retargeted(
                id,
                Branch::parse(&line.target)?,
                author,
                at,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPENED: &str = r#"{"v":1,"kind":"opened","id":"gate","title":"Land the gate","target":"main","base":"00112233445566778899aabbccddeeff00112233","author":"leandro","at":"2026-08-21T18:04:05Z"}"#;

    #[test]
    fn the_opened_line_is_the_line_it_has_always_been() -> Result<()> {
        assert_eq!(Event::parse_line(OPENED)?.to_line(), OPENED);
        Ok(())
    }

    #[test]
    fn an_opening_and_an_ending_read_back_as_they_were_written() -> Result<()> {
        let opened = Event::parse_line(OPENED)?;
        let landed = Event::landed(
            ProposalId::parse("gate")?,
            Author::parse("leandro")?,
            Timestamp::from_unix(1_787_335_445),
        );
        assert_eq!(Event::parse_line(&opened.to_line())?, opened);
        assert_eq!(Event::parse_line(&landed.to_line())?, landed);
        Ok(())
    }

    #[test]
    fn a_line_that_is_not_an_event_is_refused() {
        for raw in ["", "{"] {
            assert_eq!(
                Event::parse_line(raw),
                Err(Error::Malformed(raw.to_owned()))
            );
        }
        assert_eq!(Event::parse_line("{}"), Err(Error::Version(0)));
        assert_eq!(
            Event::parse_line(
                r#"{"v":9,"kind":"landed","id":"gate","author":"a","at":"2026-08-21T18:04:05Z"}"#
            ),
            Err(Error::Version(9))
        );
        assert_eq!(
            Event::parse_line(
                r#"{"v":1,"kind":"summoned","id":"gate","author":"a","at":"2026-08-21T18:04:05Z"}"#
            ),
            Err(Error::UnknownKind("summoned".to_owned()))
        );
    }

    #[test]
    fn an_opening_cut_from_nowhere_is_refused() {
        assert_eq!(
            Event::parse_line(
                r#"{"v":1,"kind":"opened","id":"gate","title":"t","target":"main","base":"nope","author":"a","at":"2026-08-21T18:04:05Z"}"#
            ),
            Err(Error::NoRevision("nope".to_owned()))
        );
    }

    #[test]
    fn an_ending_nobody_signed_is_refused() {
        assert_eq!(
            Event::parse_line(
                r#"{"v":1,"kind":"landed","id":"gate","author":"  ","at":"2026-08-21T18:04:05Z"}"#
            ),
            Err(Error::NoAuthor)
        );
    }
}
