//! The reasoning, attached to a proposal: the decisions it carries and the
//! lines the author wants to explain before anybody asks.
//!
//! It takes JSON rather than flags because the writer is usually an agent,
//! and every field has a ceiling in the core, so a description cannot ramble
//! whichever agent or harness produced it.

use review::{
    Anchor, Author, Chunk, FilePath, ProposalId, Rationale, Record, Side, Span, Timestamp,
};
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::store::Store;

/// The shape an author submits.
#[derive(Debug, Default, Deserialize)]
struct Description {
    #[serde(default)]
    chunks: Vec<ChunkInput>,
    #[serde(default)]
    rationale: Vec<RationaleInput>,
}

/// One reviewable decision, as it arrives.
#[derive(Debug, Default, Deserialize)]
struct ChunkInput {
    #[serde(default)]
    title: String,
    #[serde(default)]
    surface: String,
    #[serde(default)]
    before: String,
    #[serde(default)]
    after: String,
    #[serde(default)]
    decision: String,
    #[serde(default)]
    rejected: String,
    #[serde(default)]
    file: String,
    #[serde(default)]
    side: String,
    #[serde(default)]
    start: u32,
    #[serde(default)]
    end: u32,
}

/// The author explaining a few lines, as it arrives.
#[derive(Debug, Default, Deserialize)]
struct RationaleInput {
    #[serde(default)]
    file: String,
    #[serde(default)]
    side: String,
    #[serde(default)]
    start: u32,
    #[serde(default)]
    end: u32,
    #[serde(default)]
    body: String,
}

/// Read a description and write it against the head revision.
///
/// Chunks first, then rationale, in the order they were given. A refusal
/// halfway through leaves what came before it in the log, because the log is
/// append-only and half a description is still true.
///
/// # Errors
///
/// JSON this build cannot read, a field over its ceiling or carrying a
/// newline, a chunk missing its before, after or decision, or a span that
/// covers no line.
pub fn describe(
    store: &Store,
    author: &Author,
    now: Timestamp,
    id: &ProposalId,
    json: &str,
) -> Result<usize> {
    let description: Description =
        serde_json::from_str(json).map_err(|err| Error::Description(err.to_string()))?;

    let proposal = store.load(id)?;
    let head = proposal.head().sha().clone();
    let mut written = 0;

    for input in &description.chunks {
        store.annotate(&head, &Record::Chunk(chunk(input)?))?;
        written += 1;
    }

    for input in &description.rationale {
        let anchor = anchor(&input.file, &input.side, input.start, input.end)?;
        let note = Rationale::new(head.clone(), anchor, &input.body, author.clone(), now)?;

        store.annotate(&head, &Record::Rationale(note))?;
        written += 1;
    }

    Ok(written)
}

/// The shape a description takes, for somebody about to write one.
#[must_use]
pub fn template() -> &'static str {
    include_str!("describe/template.txt")
}

/// One chunk, anchored when it says where it is.
fn chunk(input: &ChunkInput) -> Result<Chunk> {
    let chunk = Chunk::new(
        &input.title,
        optional(&input.surface),
        &input.before,
        &input.after,
        &input.decision,
        optional(&input.rejected),
    )?;

    if input.file.trim().is_empty() {
        return Ok(chunk);
    }

    Ok(chunk.anchored(anchor(&input.file, &input.side, input.start, input.end)?))
}

/// Where a description points, filling in what the author left out: the new
/// side, and a single line.
fn anchor(file: &str, side: &str, start: u32, end: u32) -> Result<Anchor> {
    let side = if side.trim().is_empty() {
        Side::New
    } else {
        Side::parse(side)?
    };
    let end = if end == 0 { start } else { end };

    Ok(Anchor::new(
        FilePath::parse(file)?,
        Span::new(side, start, end)?,
    ))
}

/// A field an author may leave out entirely.
fn optional(value: &str) -> Option<&str> {
    Some(value).filter(|text| !text.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_span_defaults_to_one_line_on_the_new_side() -> Result<()> {
        let anchored = anchor("a.rs", "", 12, 0)?;

        assert_eq!(anchored.span().side(), Side::New);
        assert_eq!(anchored.span().start(), 12);
        assert_eq!(anchored.span().end(), 12);
        Ok(())
    }

    #[test]
    fn a_chunk_with_no_file_stays_unanchored() -> Result<()> {
        let input = ChunkInput {
            title: "the gate".to_owned(),
            before: "it was not there".to_owned(),
            after: "it is".to_owned(),
            decision: "put it in".to_owned(),
            ..ChunkInput::default()
        };

        assert_eq!(chunk(&input)?.anchor(), None);
        Ok(())
    }

    #[test]
    fn a_decision_over_its_ceiling_is_refused() {
        let input = ChunkInput {
            title: "the gate".to_owned(),
            before: "it was not there".to_owned(),
            after: "it is".to_owned(),
            decision: "x".repeat(201),
            ..ChunkInput::default()
        };

        let err = chunk(&input).unwrap_err();

        assert!(
            matches!(err, Error::Review(review::Error::TooLong { .. })),
            "{err}"
        );
    }

    #[test]
    fn the_template_is_the_shape_a_description_takes() {
        assert!(template().starts_with("{\n  \"chunks\": ["));
        assert!(template().ends_with("the good intentions of whoever wrote it.\n"));
    }
}
