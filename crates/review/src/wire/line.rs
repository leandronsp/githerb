//! The flat line every record serialises to, and the identity it derives.
//!
//! One struct with every field any kind can carry, in the order they appear on
//! disk. Identical records must produce identical bytes so a `cat_sort_uniq`
//! merge deduplicates them, which is why the order here is not a preference.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::identity::{Author, RecordId, Sha};
use crate::record::Kind;
use crate::span::Anchor;
use crate::timestamp::Timestamp;

/// The shape of the line an agent parses. It changes only with this number.
pub(crate) const VERSION: i64 = 1;

/// Short enough to type, long enough not to collide in a repository's worth of
/// annotations.
const ID_LENGTH: usize = 12;

/// The wire shape. Field order here is the field order on disk.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Line {
    pub v: i64,
    pub kind: String,
    pub id: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub target: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub rev: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub file: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub side: String,
    #[serde(skip_serializing_if = "is_zero")]
    pub start: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub end: i64,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub body: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub surface: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub before: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub after: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub decision: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub rejected: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub task: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub phase: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub status: String,
    #[serde(skip_serializing_if = "is_zero")]
    pub seconds: i64,
    pub author: String,
    pub at: String,
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde's skip_serializing_if hands the field by reference"
)]
fn is_zero(value: &i64) -> bool {
    *value == 0
}

/// Serialise a line. A struct of owned strings and integers cannot fail to
/// serialise: `serde_json` only refuses maps with non-string keys and floats
/// that are not finite, and this struct has neither.
pub(crate) fn render(line: &Line) -> String {
    serde_json::to_string(line).unwrap_or_default()
}

/// Name a record after what is inside it, the way git names a blob.
///
/// The argument is the record serialised with its `id` field set to the empty
/// string; the answer is the first twelve hex characters of its SHA-256.
#[must_use]
pub fn derive_id(line: &str) -> RecordId {
    let digest = Sha256::digest(line.as_bytes());
    let mut hex = String::with_capacity(ID_LENGTH);
    for byte in digest.iter().take(ID_LENGTH / 2) {
        let _ = write!(hex, "{byte:02x}");
    }
    RecordId::from_derived(hex)
}

/// The line a comment or a rationale serialises to.
pub(crate) fn note_line(
    kind: Kind,
    id: &str,
    revision: &Sha,
    anchor: &Anchor,
    body: &str,
    author: &Author,
    at: Timestamp,
) -> Line {
    Line {
        v: VERSION,
        kind: kind.as_str().to_owned(),
        id: id.to_owned(),
        rev: revision.to_string(),
        file: anchor.file().to_string(),
        side: anchor.span().side().as_str().to_owned(),
        start: i64::from(anchor.span().start()),
        end: i64::from(anchor.span().end()),
        body: body.to_owned(),
        author: author.to_string(),
        at: at.to_string(),
        ..Line::default()
    }
}

/// The line a reply serialises to.
pub(crate) fn reply_line(
    id: &str,
    target: &RecordId,
    revision: &Sha,
    body: &str,
    author: &Author,
    at: Timestamp,
) -> Line {
    Line {
        v: VERSION,
        kind: Kind::Reply.as_str().to_owned(),
        id: id.to_owned(),
        target: target.to_string(),
        rev: revision.to_string(),
        body: body.to_owned(),
        author: author.to_string(),
        at: at.to_string(),
        ..Line::default()
    }
}

/// The line a resolution serialises to.
pub(crate) fn resolution_line(id: &str, target: &RecordId, author: &Author, at: Timestamp) -> Line {
    Line {
        v: VERSION,
        kind: Kind::Resolve.as_str().to_owned(),
        id: id.to_owned(),
        target: target.to_string(),
        author: author.to_string(),
        at: at.to_string(),
        ..Line::default()
    }
}

/// The identity of a comment or a rationale.
pub(crate) fn note_id(
    kind: Kind,
    revision: &Sha,
    anchor: &Anchor,
    body: &str,
    author: &Author,
    at: Timestamp,
) -> RecordId {
    derive_id(&render(&note_line(
        kind, "", revision, anchor, body, author, at,
    )))
}

/// The identity of a reply.
pub(crate) fn reply_id(
    target: &RecordId,
    revision: &Sha,
    body: &str,
    author: &Author,
    at: Timestamp,
) -> RecordId {
    derive_id(&render(&reply_line("", target, revision, body, author, at)))
}

/// The identity of a resolution.
pub(crate) fn resolution_id(target: &RecordId, author: &Author, at: Timestamp) -> RecordId {
    derive_id(&render(&resolution_line("", target, author, at)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_identity_is_the_first_twelve_hex_characters_of_the_digest() {
        let line = r#"{"v":1,"kind":"resolve","id":"","target":"9b052da286a4","author":"claude","at":"2026-08-21T18:04:05Z"}"#;
        assert_eq!(derive_id(line).as_str(), "425ad153c4ec");
    }

    #[test]
    fn a_line_omits_what_a_kind_does_not_carry() {
        let line = Line {
            v: VERSION,
            kind: "dispatch".to_owned(),
            id: String::new(),
            rev: "9f6c1e2a3b4d5e6f708192a3b4c5d6e7f8091a2b".to_owned(),
            author: "leandro".to_owned(),
            at: "2026-08-21T18:04:05Z".to_owned(),
            ..Line::default()
        };
        assert_eq!(
            render(&line),
            r#"{"v":1,"kind":"dispatch","id":"","rev":"9f6c1e2a3b4d5e6f708192a3b4c5d6e7f8091a2b","author":"leandro","at":"2026-08-21T18:04:05Z"}"#
        );
    }
}
