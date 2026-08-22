//! A reviewable decision, and the ceilings that keep it readable.
//!
//! A chunk is the thing a person can accept or reject on its own: a file may
//! span chunks and a chunk may span files, because the unit is the decision
//! and not the path.
//!
//! The ceilings here are the whole anti-slop mechanism. A field that must fit
//! refuses to hold a paragraph, and it refuses regardless of which agent or
//! which harness wrote it. An instruction is advice; a constructor is a rule.

use std::fmt;

use crate::errors::{Error, Result};
use crate::span::Anchor;

/// A field with a ceiling on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field {
    /// What the decision is called.
    Title,
    /// What a person touches.
    Surface,
    /// How it worked.
    Before,
    /// How it works now.
    After,
    /// The call that was made.
    Decision,
    /// The alternative not taken.
    Rejected,
    /// The line an agent leaves on a work record.
    Note,
}

impl Field {
    /// The name the message uses.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Surface => "surface",
            Self::Before => "before",
            Self::After => "after",
            Self::Decision => "decision",
            Self::Rejected => "rejected",
            Self::Note => "note",
        }
    }

    /// How many characters the field may carry.
    #[must_use]
    pub fn ceiling(self) -> usize {
        match self {
            Self::Title => 80,
            Self::Surface => 60,
            Self::Decision => 200,
            Self::Before | Self::After | Self::Rejected | Self::Note => 140,
        }
    }
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Trim a capped field and refuse it if it is more than one line, or longer
/// than its ceiling. This is where prolixity dies.
///
/// # Errors
///
/// A carriage return or newline anywhere, or more characters than the ceiling.
pub fn one_line(field: Field, value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.contains(['\n', '\r']) {
        return Err(Error::NotOneLine(field));
    }
    let chars = trimmed.chars().count();
    if chars > field.ceiling() {
        return Err(Error::TooLong {
            field,
            chars,
            ceiling: field.ceiling(),
        });
    }
    Ok(trimmed.to_owned())
}

/// One reviewable decision, optionally pointing at the lines that carry it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    title: String,
    surface: Option<String>,
    before: String,
    after: String,
    decision: String,
    rejected: Option<String>,
    anchor: Option<Anchor>,
}

impl Chunk {
    /// The only way to build one. Every field is a single line and every line
    /// has a ceiling.
    ///
    /// # Errors
    ///
    /// A field over its ceiling or carrying a newline, a missing title, a
    /// missing before or after, a missing decision.
    pub fn new(
        title: &str,
        surface: Option<&str>,
        before: &str,
        after: &str,
        decision: &str,
        rejected: Option<&str>,
    ) -> Result<Self> {
        let title = one_line(Field::Title, title)?;
        let surface = optional(Field::Surface, surface)?;
        let before = one_line(Field::Before, before)?;
        let after = one_line(Field::After, after)?;
        let decision = one_line(Field::Decision, decision)?;
        let rejected = optional(Field::Rejected, rejected)?;

        if title.is_empty() {
            return Err(Error::NoTitle);
        }
        if before.is_empty() || after.is_empty() {
            return Err(Error::NoBeforeAfter);
        }
        if decision.is_empty() {
            return Err(Error::NoDecision);
        }

        Ok(Self {
            title,
            surface,
            before,
            after,
            decision,
            rejected,
            anchor: None,
        })
    }

    /// Point the chunk at the lines that carry it, so a page can take the
    /// reader there instead of asking them to find it.
    #[must_use]
    pub fn anchored(mut self, anchor: Anchor) -> Self {
        self.anchor = Some(anchor);
        self
    }

    /// What the decision is called.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// What a person touches, when the chunk says.
    #[must_use]
    pub fn surface(&self) -> Option<&str> {
        self.surface.as_deref()
    }

    /// How it worked, in one line, in product language.
    #[must_use]
    pub fn before(&self) -> &str {
        &self.before
    }

    /// How it works now, in one line.
    #[must_use]
    pub fn after(&self) -> &str {
        &self.after
    }

    /// The call that was made.
    #[must_use]
    pub fn decision(&self) -> &str {
        &self.decision
    }

    /// The alternative that was not taken, when there was one.
    #[must_use]
    pub fn rejected(&self) -> Option<&str> {
        self.rejected.as_deref()
    }

    /// Where the chunk points, when it points anywhere.
    #[must_use]
    pub fn anchor(&self) -> Option<&Anchor> {
        self.anchor.as_ref()
    }
}

/// An optional capped field: blank and absent are the same thing.
fn optional(field: Field, value: Option<&str>) -> Result<Option<String>> {
    let Some(raw) = value else { return Ok(None) };
    let trimmed = one_line(field, raw)?;
    Ok(if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::FilePath;
    use crate::span::{Side, Span};

    fn chunk() -> Result<Chunk> {
        Chunk::new(
            "Checks appear where you already look",
            Some("cmd/githerb"),
            "you had to open the browser",
            "list carries a column",
            "one column in list",
            Some("a separate checks command"),
        )
    }

    #[test]
    fn a_chunk_carries_before_after_and_the_decision() -> Result<()> {
        let chunk = chunk()?;
        assert_eq!(chunk.before(), "you had to open the browser");
        assert_eq!(chunk.after(), "list carries a column");
        assert_eq!(chunk.decision(), "one column in list");
        assert_eq!(chunk.anchor(), None);
        Ok(())
    }

    #[test]
    fn a_chunk_is_unanchored_until_it_is_told_where_it_lives() -> Result<()> {
        let anchor = Anchor::new(
            FilePath::parse("cmd/githerb/commands.go")?,
            Span::new(Side::New, 42, 47)?,
        );
        let chunk = chunk()?.anchored(anchor.clone());
        assert_eq!(chunk.anchor(), Some(&anchor));
        Ok(())
    }

    #[test]
    fn the_alternative_not_taken_is_optional() -> Result<()> {
        let chunk = Chunk::new("t", None, "b", "a", "d", None)?;
        assert_eq!(chunk.rejected(), None);
        assert_eq!(chunk.surface(), None);
        assert_eq!(
            Chunk::new("t", Some("  "), "b", "a", "d", Some(""))?.rejected(),
            None
        );
        Ok(())
    }

    #[test]
    fn a_title_over_its_ceiling_is_refused() {
        let long = "x".repeat(300);
        assert_eq!(
            Chunk::new(&long, None, "b", "a", "d", None),
            Err(Error::TooLong {
                field: Field::Title,
                chars: 300,
                ceiling: 80
            })
        );
    }

    #[test]
    fn a_decision_over_its_ceiling_is_refused() {
        let long = "x".repeat(300);
        assert_eq!(
            Chunk::new("t", None, "b", "a", &long, None),
            Err(Error::TooLong {
                field: Field::Decision,
                chars: 300,
                ceiling: 200
            })
        );
    }

    #[test]
    fn a_decision_that_is_two_lines_is_refused() {
        assert_eq!(
            Chunk::new("t", None, "b", "a", "one\ntwo", None),
            Err(Error::NotOneLine(Field::Decision))
        );
    }

    #[test]
    fn a_ceiling_counts_characters_and_not_bytes() -> Result<()> {
        let eighty = "é".repeat(80);
        assert_eq!(
            Chunk::new(&eighty, None, "b", "a", "d", None)?.title(),
            eighty
        );
        Ok(())
    }

    #[test]
    fn a_chunk_missing_what_it_is_for_is_refused() {
        assert_eq!(
            Chunk::new("", None, "b", "a", "d", None),
            Err(Error::NoTitle)
        );
        assert_eq!(
            Chunk::new("t", None, "", "a", "d", None),
            Err(Error::NoBeforeAfter)
        );
        assert_eq!(
            Chunk::new("t", None, "b", "", "d", None),
            Err(Error::NoBeforeAfter)
        );
        assert_eq!(
            Chunk::new("t", None, "b", "a", "", None),
            Err(Error::NoDecision)
        );
    }
}
