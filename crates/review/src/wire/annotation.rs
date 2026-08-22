//! A record to its line, and a line back to a record.
//!
//! Parsing runs the real constructors, so every rule in the domain applies on
//! read exactly as it does on write. A version this build does not speak is
//! refused; a kind it does not know is reported by name, and the store skips
//! those rather than failing the load.

use super::line::{Line, VERSION, note_line, render, reply_line, resolution_line};
use crate::check::{Check, CheckName, CheckStatus};
use crate::chunk::Chunk;
use crate::comment::Comment;
use crate::errors::{Error, Result};
use crate::identity::{Author, FilePath, RecordId, Sha};
use crate::rationale::Rationale;
use crate::record::{Kind, Record};
use crate::reply::Reply;
use crate::resolution::Resolution;
use crate::span::{Anchor, Side, Span};
use crate::timestamp::Timestamp;
use crate::work::{Dispatch, Phase, Task, Work};

impl Record {
    /// Render the record as the single line it is stored as, with no newline.
    #[must_use]
    pub fn to_line(&self) -> String {
        render(&self.line())
    }

    fn line(&self) -> Line {
        match self {
            Self::Comment(comment) => note_line(
                Kind::Comment,
                comment.id().as_str(),
                comment.revision(),
                comment.anchor(),
                comment.body(),
                comment.author(),
                comment.at(),
            ),
            Self::Rationale(rationale) => note_line(
                Kind::Rationale,
                rationale.id().as_str(),
                rationale.revision(),
                rationale.anchor(),
                rationale.body(),
                rationale.author(),
                rationale.at(),
            ),
            Self::Reply(reply) => reply_line(
                reply.id().as_str(),
                reply.target(),
                reply.revision(),
                reply.body(),
                reply.author(),
                reply.at(),
            ),
            Self::Resolve(resolution) => resolution_line(
                resolution.id().as_str(),
                resolution.target(),
                resolution.author(),
                resolution.at(),
            ),
            Self::Check(check) => Line {
                v: VERSION,
                kind: Kind::Check.as_str().to_owned(),
                rev: check.revision().to_string(),
                name: check.name().to_string(),
                status: check.status().as_str().to_owned(),
                seconds: i64::from(check.seconds()),
                author: check.author().to_string(),
                at: check.at().to_string(),
                ..Line::default()
            },
            Self::Chunk(chunk) => Line {
                v: VERSION,
                kind: Kind::Chunk.as_str().to_owned(),
                file: chunk
                    .anchor()
                    .map(|a| a.file().to_string())
                    .unwrap_or_default(),
                side: chunk
                    .anchor()
                    .map(|a| a.span().side().as_str().to_owned())
                    .unwrap_or_default(),
                start: chunk.anchor().map_or(0, |a| i64::from(a.span().start())),
                end: chunk.anchor().map_or(0, |a| i64::from(a.span().end())),
                title: chunk.title().to_owned(),
                surface: chunk.surface().unwrap_or_default().to_owned(),
                before: chunk.before().to_owned(),
                after: chunk.after().to_owned(),
                decision: chunk.decision().to_owned(),
                rejected: chunk.rejected().unwrap_or_default().to_owned(),
                ..Line::default()
            },
            Self::Work(work) => Line {
                v: VERSION,
                kind: Kind::Work.as_str().to_owned(),
                rev: work.revision().to_string(),
                body: work.note().unwrap_or_default().to_owned(),
                task: work.task().as_str().to_owned(),
                phase: work.phase().as_str().to_owned(),
                author: work.agent().to_string(),
                at: work.at().to_string(),
                ..Line::default()
            },
            Self::Dispatch(dispatch) => Line {
                v: VERSION,
                kind: Kind::Dispatch.as_str().to_owned(),
                rev: dispatch.revision().to_string(),
                author: dispatch.author().to_string(),
                at: dispatch.at().to_string(),
                ..Line::default()
            },
        }
    }

    /// Read one line of the annotation log.
    ///
    /// # Errors
    ///
    /// Text that is not JSON, a version this build does not speak, a kind it
    /// does not know, or a record any constructor here refuses.
    pub fn parse_line(raw: &str) -> Result<Self> {
        let line: Line = serde_json::from_str(raw).map_err(|_| Error::Malformed(raw.to_owned()))?;
        if line.v != VERSION {
            return Err(Error::Version(line.v));
        }
        // A chunk is the one kind with no moment of its own: it describes the
        // work, not something that happened to it.
        let at = if line.kind == Kind::Chunk.as_str() {
            Timestamp::from_unix(0)
        } else {
            Timestamp::parse(&line.at)
                .map_err(|_| Error::Malformed(format!("timestamp {}", line.at)))?
        };

        match Kind::parse(&line.kind)? {
            Kind::Comment => Ok(Self::Comment(Comment::restore(
                stored_id(&line)?,
                Sha::parse(&line.rev)?,
                anchor(&line)?,
                &line.body,
                Author::parse(&line.author)?,
                at,
            )?)),
            Kind::Rationale => Ok(Self::Rationale(Rationale::restore(
                stored_id(&line)?,
                Sha::parse(&line.rev)?,
                anchor(&line)?,
                &line.body,
                Author::parse(&line.author)?,
                at,
            )?)),
            Kind::Reply => Ok(Self::Reply(Reply::restore(
                stored_id(&line)?,
                RecordId::parse(&line.target)?,
                Sha::parse(&line.rev)?,
                &line.body,
                Author::parse(&line.author)?,
                at,
            )?)),
            Kind::Resolve => Ok(Self::Resolve(Resolution::restore(
                stored_id(&line)?,
                RecordId::parse(&line.target)?,
                Author::parse(&line.author)?,
                at,
            ))),
            Kind::Check => Ok(Self::Check(Check::new(
                CheckName::parse(&line.name)?,
                CheckStatus::parse(&line.status)?,
                Sha::parse(&line.rev)?,
                seconds(line.seconds)?,
                Author::parse(&line.author)?,
                at,
            ))),
            Kind::Chunk => parse_chunk(&line),
            Kind::Work => Ok(Self::Work(Work::new(
                Sha::parse(&line.rev)?,
                Task::parse(&line.task)?,
                Phase::parse(&line.phase)?,
                Author::parse(&line.author)?,
                optional(&line.body),
                at,
            )?)),
            Kind::Dispatch => Ok(Self::Dispatch(Dispatch::new(
                Sha::parse(&line.rev)?,
                Author::parse(&line.author)?,
                at,
            ))),
        }
    }
}

/// A chunk stays unanchored until the line names a file.
fn parse_chunk(line: &Line) -> Result<Record> {
    let chunk = Chunk::new(
        &line.title,
        optional(&line.surface),
        &line.before,
        &line.after,
        &line.decision,
        optional(&line.rejected),
    )?;
    if line.file.is_empty() {
        return Ok(Record::Chunk(chunk));
    }
    Ok(Record::Chunk(chunk.anchored(anchor(line)?)))
}

fn anchor(line: &Line) -> Result<Anchor> {
    let span = Span::new(
        Side::parse(&line.side)?,
        lines(line.start, line.start, line.end)?,
        lines(line.end, line.start, line.end)?,
    )?;
    Ok(Anchor::new(FilePath::parse(&line.file)?, span))
}

/// A line number the wire carries as a signed integer. Anything a `u32` cannot
/// hold is a span that covers nothing.
fn lines(value: i64, start: i64, end: i64) -> Result<u32> {
    u32::try_from(value).map_err(|_| Error::EmptySpan { start, end })
}

fn seconds(value: i64) -> Result<u32> {
    u32::try_from(value).map_err(|_| Error::Malformed(format!("{value} seconds")))
}

/// The identity a record was written with. A record from another build may
/// carry an id this one would not derive, and that id is still the one every
/// resolution and reply points at, so it is kept rather than recomputed.
fn stored_id(line: &Line) -> Result<Option<RecordId>> {
    if line.id.is_empty() {
        return Ok(None);
    }
    RecordId::parse(&line.id).map(Some)
}

/// A blank string on the wire is an absent value in the domain.
fn optional(value: &str) -> Option<&str> {
    if value.is_empty() { None } else { Some(value) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &str = "9f6c1e2a3b4d5e6f708192a3b4c5d6e7f8091a2b";
    const COMMENT: &str = r#"{"v":1,"kind":"comment","id":"9b052da286a4","rev":"9f6c1e2a3b4d5e6f708192a3b4c5d6e7f8091a2b","file":"internal/app/land.go","side":"new","start":42,"end":47,"body":"this leaks the handle when init fails","author":"leandro","at":"2026-08-21T18:04:05Z"}"#;

    fn comment() -> Result<Record> {
        Ok(Record::Comment(Comment::new(
            Sha::parse(HEAD)?,
            Anchor::new(
                FilePath::parse("internal/app/land.go")?,
                Span::new(Side::New, 42, 47)?,
            ),
            "this leaks the handle when init fails",
            Author::parse("leandro")?,
            Timestamp::parse("2026-08-21T18:04:05Z")
                .map_err(|_| Error::Malformed("fixture".to_owned()))?,
        )?))
    }

    #[test]
    fn a_record_is_one_line_and_only_one() -> Result<()> {
        let line = comment()?.to_line();
        assert_eq!(line.lines().count(), 1);
        assert_eq!(line, COMMENT);
        Ok(())
    }

    #[test]
    fn a_record_read_back_is_the_record_that_was_written() -> Result<()> {
        assert_eq!(Record::parse_line(&comment()?.to_line())?, comment()?);
        Ok(())
    }

    #[test]
    fn a_resolution_and_a_check_read_back_as_they_were_written() -> Result<()> {
        let at = Timestamp::from_unix(1_787_335_445);
        let resolution = Record::Resolve(Resolution::new(
            RecordId::parse("9b052da286a4")?,
            Author::parse("claude")?,
            at,
        ));
        let check = Record::Check(Check::new(
            CheckName::parse("suite")?,
            CheckStatus::Passed,
            Sha::parse(HEAD)?,
            41,
            Author::parse("githerb-ci@laptop")?,
            at,
        ));
        assert_eq!(Record::parse_line(&resolution.to_line())?, resolution);
        assert_eq!(Record::parse_line(&check.to_line())?, check);
        Ok(())
    }

    #[test]
    fn a_line_that_is_not_a_record_is_refused() {
        for raw in ["", "not json", "{", "null", "42"] {
            assert_eq!(
                Record::parse_line(raw),
                Err(Error::Malformed(raw.to_owned()))
            );
        }
    }

    #[test]
    fn a_version_this_build_does_not_speak_is_refused_and_never_skipped() {
        assert_eq!(
            Record::parse_line(
                r#"{"v":99,"kind":"comment","id":"","author":"a","at":"2026-08-21T18:04:05Z"}"#
            ),
            Err(Error::Version(99))
        );
        assert_eq!(Record::parse_line("{}"), Err(Error::Version(0)));
    }

    #[test]
    fn a_kind_this_build_does_not_know_is_refused_by_name() {
        assert_eq!(
            Record::parse_line(
                r#"{"v":1,"kind":"telepathy","id":"","author":"a","at":"2026-08-21T18:04:05Z"}"#
            ),
            Err(Error::UnknownKind("telepathy".to_owned()))
        );
        assert_eq!(
            Record::parse_line(r#"{"v":1,"id":"","author":"a","at":"2026-08-21T18:04:05Z"}"#),
            Err(Error::UnknownKind(String::new()))
        );
    }

    #[test]
    fn a_bad_timestamp_is_not_a_record() {
        assert_eq!(
            Record::parse_line(r#"{"v":1,"kind":"comment","id":"","author":"a","at":"yesterday"}"#),
            Err(Error::Malformed("timestamp yesterday".to_owned()))
        );
    }

    #[test]
    fn a_comment_missing_its_span_is_refused() {
        assert_eq!(
            Record::parse_line(
                r#"{"v":1,"kind":"comment","id":"","rev":"9f6c1e2a3b4d5e6f708192a3b4c5d6e7f8091a2b","file":"a.go","body":"x","author":"a","at":"2026-08-21T18:04:05Z"}"#
            ),
            Err(Error::UnknownSide(String::new()))
        );
    }

    #[test]
    fn a_note_on_a_line_that_cannot_exist_is_refused() {
        assert_eq!(
            Record::parse_line(
                r#"{"v":1,"kind":"comment","id":"","rev":"9f6c1e2a3b4d5e6f708192a3b4c5d6e7f8091a2b","file":"a.go","side":"new","start":-5,"end":3,"body":"x","author":"a","at":"2026-08-21T18:04:05Z"}"#
            ),
            Err(Error::EmptySpan { start: -5, end: 3 })
        );
        assert_eq!(
            Record::parse_line(
                r#"{"v":1,"kind":"comment","id":"","rev":"9f6c1e2a3b4d5e6f708192a3b4c5d6e7f8091a2b","file":"a.go","side":"new","start":9,"end":4,"body":"x","author":"a","at":"2026-08-21T18:04:05Z"}"#
            ),
            Err(Error::EmptySpan { start: 9, end: 4 })
        );
    }

    #[test]
    fn a_resolution_with_no_target_is_refused() {
        assert_eq!(
            Record::parse_line(
                r#"{"v":1,"kind":"resolve","id":"","author":"claude","at":"2026-08-21T18:04:05Z"}"#
            ),
            Err(Error::NoTarget)
        );
    }

    #[test]
    fn a_check_that_ran_for_negative_seconds_is_not_a_record() {
        assert_eq!(
            Record::parse_line(
                r#"{"v":1,"kind":"check","id":"","rev":"9f6c1e2a3b4d5e6f708192a3b4c5d6e7f8091a2b","name":"suite","status":"passed","seconds":-3,"author":"a","at":"2026-08-21T18:04:05Z"}"#
            ),
            Err(Error::Malformed("-3 seconds".to_owned()))
        );
    }

    #[test]
    fn an_unanchored_chunk_stays_unanchored_through_the_wire() -> Result<()> {
        let line = r#"{"v":1,"kind":"chunk","id":"","title":"t","before":"b","after":"a","decision":"d","author":"","at":""}"#;
        let Record::Chunk(chunk) = Record::parse_line(line)? else {
            panic!("expected a chunk")
        };
        assert_eq!(chunk.anchor(), None);
        Ok(())
    }
}
