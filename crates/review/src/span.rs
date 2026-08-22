//! Where an annotation points: a side of the diff, a range of lines, a file.
//!
//! A span is inclusive at both ends and a single line is a span whose start
//! and end are equal, so there is one shape to read and one shape to store.

use std::fmt;

use crate::errors::{Error, Result};
use crate::identity::FilePath;

/// Which column of a diff a span belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Side {
    /// The file as it was. A note on a deleted line is here.
    Old,
    /// The file as it is after the change.
    New,
}

impl Side {
    /// Read a side off the wire or a command line.
    ///
    /// # Errors
    ///
    /// Anything that is not `old` or `new`.
    pub fn parse(raw: &str) -> Result<Self> {
        match raw {
            "old" => Ok(Self::Old),
            "new" => Ok(Self::New),
            _ => Err(Error::UnknownSide(raw.to_owned())),
        }
    }

    /// The word the wire format uses.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Old => "old",
            Self::New => "new",
        }
    }
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A range of lines on one side of a diff, inclusive at both ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    side: Side,
    start: u32,
    end: u32,
}

impl Span {
    /// The only way to build one, so an empty range cannot exist.
    ///
    /// # Errors
    ///
    /// A range that starts before line one, or ends before it starts.
    pub fn new(side: Side, start: u32, end: u32) -> Result<Self> {
        if start < 1 || end < start {
            return Err(Error::EmptySpan {
                start: i64::from(start),
                end: i64::from(end),
            });
        }
        Ok(Self { side, start, end })
    }

    /// Which column of the diff the span is on.
    #[must_use]
    pub fn side(self) -> Side {
        self.side
    }

    /// The first line the span covers.
    #[must_use]
    pub fn start(self) -> u32 {
        self.start
    }

    /// The last line the span covers.
    #[must_use]
    pub fn end(self) -> u32 {
        self.end
    }

    /// How many lines the span covers.
    #[must_use]
    pub fn lines(self) -> u32 {
        self.end - self.start + 1
    }
}

/// A file and the lines in it that an annotation is about.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Anchor {
    file: FilePath,
    span: Span,
}

impl Anchor {
    /// Point at a range of lines of a file.
    #[must_use]
    pub fn new(file: FilePath, span: Span) -> Self {
        Self { file, span }
    }

    /// The file.
    #[must_use]
    pub fn file(&self) -> &FilePath {
        &self.file
    }

    /// The lines.
    #[must_use]
    pub fn span(&self) -> Span {
        self.span
    }
}

impl fmt::Display for Anchor {
    /// `internal/app/land.go:42-47 new`, the way a brief writes it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.file, self.span.start())?;
        if self.span.lines() > 1 {
            write!(f, "-{}", self.span.end())?;
        }
        write!(f, " {}", self.span.side())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_side_is_the_old_one_or_the_new_one() -> Result<()> {
        assert_eq!(Side::parse("old")?, Side::Old);
        assert_eq!(Side::parse("new")?, Side::New);
        Ok(())
    }

    #[test]
    fn a_span_on_no_known_side_is_refused() {
        for raw in ["", "left", "NEW", " new"] {
            assert_eq!(Side::parse(raw), Err(Error::UnknownSide(raw.to_owned())));
        }
    }

    #[test]
    fn a_span_covers_at_least_one_line() -> Result<()> {
        let span = Span::new(Side::New, 42, 47)?;
        assert_eq!((span.start(), span.end(), span.lines()), (42, 47, 6));
        assert_eq!(Span::new(Side::New, 42, 42)?.lines(), 1);
        Ok(())
    }

    #[test]
    fn a_span_that_starts_at_zero_or_ends_early_is_refused() {
        assert_eq!(
            Span::new(Side::New, 0, 3),
            Err(Error::EmptySpan { start: 0, end: 3 })
        );
        assert_eq!(
            Span::new(Side::New, 9, 4),
            Err(Error::EmptySpan { start: 9, end: 4 })
        );
    }

    #[test]
    fn an_anchor_reads_as_file_lines_and_side() -> Result<()> {
        let file = FilePath::parse("internal/app/land.go")?;
        let many = Anchor::new(file.clone(), Span::new(Side::New, 42, 47)?);
        let one = Anchor::new(file, Span::new(Side::Old, 42, 42)?);
        assert_eq!(many.to_string(), "internal/app/land.go:42-47 new");
        assert_eq!(one.to_string(), "internal/app/land.go:42 old");
        Ok(())
    }
}
