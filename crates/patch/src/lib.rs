//! A unified diff parsed into lines an annotation can point at.
//!
//! No git, no I/O, only text. `git diff` produces the text, this crate turns
//! it into files, hunks and numbered lines so a note can say "new side, line
//! 12 of README.md" and the page can find that exact line again.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use std::fmt;

/// Which side of the diff a line number counts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    /// The file as it was before the change.
    Old,
    /// The file as it is after the change.
    New,
}

/// What a line in a hunk does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// Present on both sides, unchanged.
    Context,
    /// Present only on the new side.
    Added,
    /// Present only on the old side.
    Removed,
}

/// One line of a hunk, numbered on the sides it exists on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    kind: LineKind,
    old: Option<u32>,
    new: Option<u32>,
    text: String,
}

impl Line {
    /// What the line does.
    #[must_use]
    pub fn kind(&self) -> LineKind {
        self.kind
    }

    /// The line number on the old side, if the line exists there.
    #[must_use]
    pub fn old_number(&self) -> Option<u32> {
        self.old
    }

    /// The line number on the new side, if the line exists there.
    #[must_use]
    pub fn new_number(&self) -> Option<u32> {
        self.new
    }

    /// The line number on the given side, if the line exists there.
    #[must_use]
    pub fn number(&self, side: Side) -> Option<u32> {
        match side {
            Side::Old => self.old,
            Side::New => self.new,
        }
    }

    /// The text of the line without the leading marker and trailing newline.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// A run of lines introduced by an `@@` header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    header: String,
    lines: Vec<Line>,
}

impl Hunk {
    /// The `@@ -a,b +c,d @@ section` line, verbatim.
    #[must_use]
    pub fn header(&self) -> &str {
        &self.header
    }

    /// The lines of the hunk, in order.
    #[must_use]
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }
}

/// How a file came to be in the diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// Existed before and after, under the same name.
    Modified,
    /// Did not exist before.
    Added,
    /// Does not exist after.
    Deleted,
    /// Moved from the old path to the new one.
    Renamed {
        /// Where it was.
        from: String,
    },
}

/// One file's part of the diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    path: String,
    change: Change,
    binary: bool,
    hunks: Vec<Hunk>,
}

impl FileDiff {
    /// The path the file has after the change, or had before if it was deleted.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// How the file came to be in the diff.
    #[must_use]
    pub fn change(&self) -> &Change {
        &self.change
    }

    /// Whether git declined to show the content.
    #[must_use]
    pub fn is_binary(&self) -> bool {
        self.binary
    }

    /// The hunks, in order.
    #[must_use]
    pub fn hunks(&self) -> &[Hunk] {
        &self.hunks
    }

    /// Every line of every hunk, in order.
    pub fn lines(&self) -> impl Iterator<Item = &Line> {
        self.hunks.iter().flat_map(|hunk| hunk.lines.iter())
    }

    /// The line carrying that number on that side, if the diff shows it.
    #[must_use]
    pub fn line(&self, side: Side, number: u32) -> Option<&Line> {
        self.lines().find(|line| line.number(side) == Some(number))
    }

    /// How many lines were added.
    #[must_use]
    pub fn added(&self) -> usize {
        self.lines()
            .filter(|line| line.kind == LineKind::Added)
            .count()
    }

    /// How many lines were removed.
    #[must_use]
    pub fn removed(&self) -> usize {
        self.lines()
            .filter(|line| line.kind == LineKind::Removed)
            .count()
    }
}

/// A whole unified diff.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Patch {
    files: Vec<FileDiff>,
}

impl Patch {
    /// The files, in the order git listed them.
    #[must_use]
    pub fn files(&self) -> &[FileDiff] {
        &self.files
    }

    /// The file at that path, if the diff touches it.
    #[must_use]
    pub fn file(&self, path: &str) -> Option<&FileDiff> {
        self.files.iter().find(|file| file.path == path)
    }

    /// Lines added across every file.
    #[must_use]
    pub fn added(&self) -> usize {
        self.files.iter().map(FileDiff::added).sum()
    }

    /// Lines removed across every file.
    #[must_use]
    pub fn removed(&self) -> usize {
        self.files.iter().map(FileDiff::removed).sum()
    }
}

/// Why a diff could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// An `@@` header that does not say where the hunk starts.
    BadHunkHeader(String),
    /// A hunk line before any `@@` header.
    LineOutsideHunk(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadHunkHeader(header) => write!(f, "unreadable hunk header: {header}"),
            Self::LineOutsideHunk(line) => write!(f, "diff line outside any hunk: {line}"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parse the output of `git diff`.
///
/// # Errors
///
/// Fails on a hunk header that cannot be read, or a changed line that arrives
/// before any hunk header.
pub fn parse(diff: &str) -> Result<Patch, ParseError> {
    let mut files: Vec<FileDiff> = Vec::new();
    let mut old_line = 0_u32;
    let mut new_line = 0_u32;

    for raw in diff.split_inclusive('\n') {
        let line = raw.strip_suffix('\n').unwrap_or(raw);
        let line = line.strip_suffix('\r').unwrap_or(line);

        if let Some(rest) = line.strip_prefix("diff --git ") {
            files.push(FileDiff {
                path: new_path_from_header(rest),
                change: Change::Modified,
                binary: false,
                hunks: Vec::new(),
            });
            continue;
        }

        let Some(file) = files.last_mut() else {
            continue;
        };

        if let Some(header) = line.strip_prefix("@@ ") {
            let (old_start, new_start) =
                hunk_starts(header).ok_or_else(|| ParseError::BadHunkHeader(line.to_owned()))?;
            old_line = old_start;
            new_line = new_start;
            file.hunks.push(Hunk {
                header: line.to_owned(),
                lines: Vec::new(),
            });
            continue;
        }

        if line.starts_with("new file mode") {
            file.change = Change::Added;
        } else if line.starts_with("deleted file mode") {
            file.change = Change::Deleted;
        } else if let Some(from) = line.strip_prefix("rename from ") {
            file.change = Change::Renamed {
                from: from.to_owned(),
            };
        } else if let Some(to) = line.strip_prefix("rename to ") {
            to.clone_into(&mut file.path);
        } else if let Some(path) = line.strip_prefix("--- ") {
            if file.change == Change::Deleted {
                strip_prefix_a_b(path).clone_into(&mut file.path);
            }
        } else if let Some(path) = line.strip_prefix("+++ ") {
            if path != "/dev/null" {
                strip_prefix_a_b(path).clone_into(&mut file.path);
            }
        } else if line.starts_with("Binary files ") || line.starts_with("GIT binary patch") {
            file.binary = true;
        } else if let Some(hunk) = file.hunks.last_mut() {
            push_hunk_line(hunk, line, &mut old_line, &mut new_line)?;
        } else if line.starts_with(['+', '-', ' ']) {
            return Err(ParseError::LineOutsideHunk(line.to_owned()));
        }
    }

    Ok(Patch { files })
}

fn push_hunk_line(
    hunk: &mut Hunk,
    line: &str,
    old_line: &mut u32,
    new_line: &mut u32,
) -> Result<(), ParseError> {
    let (kind, text) = match line.split_at_checked(1) {
        Some(("+", text)) => (LineKind::Added, text),
        Some(("-", text)) => (LineKind::Removed, text),
        Some((" ", text)) => (LineKind::Context, text),
        Some(("\\", _)) => return Ok(()),
        None => (LineKind::Context, ""),
        Some(_) => return Err(ParseError::LineOutsideHunk(line.to_owned())),
    };
    let (old, new) = match kind {
        LineKind::Context => (Some(*old_line), Some(*new_line)),
        LineKind::Added => (None, Some(*new_line)),
        LineKind::Removed => (Some(*old_line), None),
    };
    if old.is_some() {
        *old_line += 1;
    }
    if new.is_some() {
        *new_line += 1;
    }
    hunk.lines.push(Line {
        kind,
        old,
        new,
        text: text.to_owned(),
    });
    Ok(())
}

/// `-1,4 +1,4 @@ section` to `(1, 1)`.
fn hunk_starts(header: &str) -> Option<(u32, u32)> {
    let mut parts = header.split_whitespace();
    let old = parts.next()?.strip_prefix('-')?;
    let new = parts.next()?.strip_prefix('+')?;
    Some((start_of(old)?, start_of(new)?))
}

/// `12,3` or `12` to `12`. A zero-length range (`0,0`) starts at line 1.
fn start_of(range: &str) -> Option<u32> {
    let (start, count) = range.split_once(',').unwrap_or((range, "1"));
    let start: u32 = start.parse().ok()?;
    let count: u32 = count.parse().ok()?;
    Some(if count == 0 { start + 1 } else { start })
}

/// `a/README.md b/README.md` to `README.md`.
fn new_path_from_header(rest: &str) -> String {
    let (_, new) = rest.split_once(" b/").unwrap_or(("", rest));
    new.to_owned()
}

fn strip_prefix_a_b(path: &str) -> &str {
    path.strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODIFIED_AND_ADDED: &str = "\
diff --git a/README.md b/README.md
index 3b1ba0c..4f1abde 100644
--- a/README.md
+++ b/README.md
@@ -1,4 +1,4 @@
-# githerb
+# githerb (demo)

 Code review and a gate for trunk, in one binary, with no server.

diff --git a/cmd/githerb/extra.go b/cmd/githerb/extra.go
new file mode 100644
index 0000000..0b5c82e
--- /dev/null
+++ b/cmd/githerb/extra.go
@@ -0,0 +1,3 @@
+package main
+
+func extra() int { return 1 }
";

    #[test]
    fn reads_every_file_with_its_change() -> Result<(), ParseError> {
        let patch = parse(MODIFIED_AND_ADDED)?;
        let changes: Vec<(&str, &Change)> = patch
            .files()
            .iter()
            .map(|file| (file.path(), file.change()))
            .collect();
        assert_eq!(
            changes,
            vec![
                ("README.md", &Change::Modified),
                ("cmd/githerb/extra.go", &Change::Added)
            ]
        );
        Ok(())
    }

    #[test]
    fn numbers_lines_on_the_sides_they_exist_on() -> Result<(), ParseError> {
        let patch = parse(MODIFIED_AND_ADDED)?;
        let readme = patch.file("README.md").unwrap();
        let numbered: Vec<(LineKind, Option<u32>, Option<u32>)> = readme
            .lines()
            .map(|line| (line.kind(), line.old_number(), line.new_number()))
            .collect();
        assert_eq!(
            numbered,
            vec![
                (LineKind::Removed, Some(1), None),
                (LineKind::Added, None, Some(1)),
                (LineKind::Context, Some(2), Some(2)),
                (LineKind::Context, Some(3), Some(3)),
                (LineKind::Context, Some(4), Some(4)),
            ]
        );
        Ok(())
    }

    #[test]
    fn an_added_file_starts_counting_at_one() -> Result<(), ParseError> {
        let patch = parse(MODIFIED_AND_ADDED)?;
        let extra = patch.file("cmd/githerb/extra.go").unwrap();
        assert_eq!(
            extra.line(Side::New, 3).map(Line::text),
            Some("func extra() int { return 1 }")
        );
        assert_eq!(extra.line(Side::Old, 1), None);
        Ok(())
    }

    #[test]
    fn counts_added_and_removed() -> Result<(), ParseError> {
        let patch = parse(MODIFIED_AND_ADDED)?;
        assert_eq!((patch.added(), patch.removed()), (4, 1));
        assert_eq!(
            (
                patch.file("README.md").unwrap().added(),
                patch.file("README.md").unwrap().removed()
            ),
            (1, 1)
        );
        Ok(())
    }

    #[test]
    fn keeps_the_hunk_header_verbatim() -> Result<(), ParseError> {
        let patch = parse(MODIFIED_AND_ADDED)?;
        assert_eq!(
            patch.file("README.md").unwrap().hunks()[0].header(),
            "@@ -1,4 +1,4 @@"
        );
        Ok(())
    }

    #[test]
    fn a_second_hunk_restarts_the_numbering() -> Result<(), ParseError> {
        let diff = "\
diff --git a/f b/f
--- a/f
+++ b/f
@@ -1,2 +1,2 @@
-a
+A
 b
@@ -10,2 +10,3 @@ fn section
 j
+J
 k
";
        let patch = parse(diff)?;
        let file = patch.file("f").unwrap();
        assert_eq!(file.hunks()[1].header(), "@@ -10,2 +10,3 @@ fn section");
        assert_eq!(file.line(Side::New, 11).map(Line::text), Some("J"));
        assert_eq!(file.line(Side::Old, 11).map(Line::text), Some("k"));
        Ok(())
    }

    #[test]
    fn deleted_renamed_and_binary_files() -> Result<(), ParseError> {
        let diff = "\
diff --git a/gone.txt b/gone.txt
deleted file mode 100644
index e69de29..0000000
--- a/gone.txt
+++ /dev/null
@@ -1 +0,0 @@
-bye
diff --git a/old.rs b/new.rs
similarity index 90%
rename from old.rs
rename to new.rs
index 1111111..2222222 100644
--- a/old.rs
+++ b/new.rs
@@ -1 +1 @@
-x
+y
diff --git a/logo.png b/logo.png
new file mode 100644
index 0000000..3333333
Binary files /dev/null and b/logo.png differ
";
        let patch = parse(diff)?;
        let summary: Vec<(&str, &Change, bool)> = patch
            .files()
            .iter()
            .map(|file| (file.path(), file.change(), file.is_binary()))
            .collect();
        assert_eq!(
            summary,
            vec![
                ("gone.txt", &Change::Deleted, false),
                (
                    "new.rs",
                    &Change::Renamed {
                        from: "old.rs".to_owned()
                    },
                    false
                ),
                ("logo.png", &Change::Added, true),
            ]
        );
        assert_eq!(
            patch
                .file("gone.txt")
                .unwrap()
                .line(Side::Old, 1)
                .map(Line::text),
            Some("bye")
        );
        Ok(())
    }

    #[test]
    fn a_missing_trailing_newline_marker_is_not_a_line() -> Result<(), ParseError> {
        let diff = "\
diff --git a/f b/f
--- a/f
+++ b/f
@@ -1 +1 @@
-a
\\ No newline at end of file
+b
\\ No newline at end of file
";
        let patch = parse(diff)?;
        assert_eq!(patch.file("f").unwrap().lines().count(), 2);
        Ok(())
    }

    #[test]
    fn an_empty_diff_has_no_files() -> Result<(), ParseError> {
        assert_eq!(parse("")?.files().len(), 0);
        Ok(())
    }

    #[test]
    fn a_bad_hunk_header_is_refused() {
        let diff = "diff --git a/f b/f\n--- a/f\n+++ b/f\n@@ nonsense @@\n+x\n";
        assert_eq!(
            parse(diff),
            Err(ParseError::BadHunkHeader("@@ nonsense @@".to_owned()))
        );
    }

    #[test]
    fn a_changed_line_before_any_hunk_is_refused() {
        let diff = "diff --git a/f b/f\n--- a/f\n+++ b/f\n+x\n";
        assert_eq!(
            parse(diff),
            Err(ParseError::LineOutsideHunk("+x".to_owned()))
        );
    }
}
