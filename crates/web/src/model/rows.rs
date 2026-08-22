//! The diff, one row at a time, with everything a row needs already on it.
//!
//! A row is built once and carries its own id, its classes, the decisions it
//! starts, the explanations it ends and the threads that sit under it. The
//! renderer walks rows and writes them out; it never asks a question that
//! costs a scan.

use std::collections::HashMap;

use patch::{FileDiff, LineKind};
use review::Side;

use crate::model::threads::{Anchors, Thread};

/// What a row of the diff does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    /// Present on both sides, unchanged.
    Context,
    /// Present only on the new side.
    Added,
    /// Present only on the old side.
    Removed,
}

impl RowKind {
    /// The class the row carries, and nothing for a context row: context is
    /// the default look, and the bytes it would cost are paid on every line
    /// of every file.
    #[must_use]
    pub fn class(self) -> Option<&'static str> {
        match self {
            Self::Context => None,
            Self::Added => Some("add"),
            Self::Removed => Some("del"),
        }
    }
}

/// One line of the diff, ready to render.
#[derive(Debug, Clone)]
pub struct Row {
    id: String,
    kind: RowKind,
    old: Option<u32>,
    new: Option<u32>,
    text: String,
    noted: bool,
    decisions: Vec<u32>,
    why: Vec<String>,
    threads: Vec<usize>,
}

impl Row {
    /// The row's dom id, `L-<file>-<row>`.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// What the row does.
    #[must_use]
    pub fn kind(&self) -> RowKind {
        self.kind
    }

    /// The line number on the old side, if the row has one.
    #[must_use]
    pub fn old_number(&self) -> Option<u32> {
        self.old
    }

    /// The line number on the new side, if the row has one.
    #[must_use]
    pub fn new_number(&self) -> Option<u32> {
        self.new
    }

    /// The code.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Whether an open note covers this line.
    #[must_use]
    pub fn noted(&self) -> bool {
        self.noted
    }

    /// The decisions that start here, by their number in the rail.
    #[must_use]
    pub fn decisions(&self) -> &[u32] {
        &self.decisions
    }

    /// The author's explanations that end here.
    #[must_use]
    pub fn why(&self) -> &[String] {
        &self.why
    }

    /// The threads that render under this row, by position in the page's
    /// thread list.
    #[must_use]
    pub fn threads(&self) -> &[usize] {
        &self.threads
    }
}

/// A run of rows under one `@@` header.
#[derive(Debug, Clone)]
pub struct HunkView {
    header: String,
    rows: Vec<Row>,
}

impl HunkView {
    /// The `@@ -a,b +c,d @@` line, verbatim.
    #[must_use]
    pub fn header(&self) -> &str {
        &self.header
    }

    /// The rows, in order.
    #[must_use]
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }
}

/// One file's part of the diff, and how much of it the page shows.
#[derive(Debug, Clone)]
pub struct FileView {
    index: usize,
    path: String,
    added: usize,
    removed: usize,
    lines: usize,
    binary: bool,
    noted: bool,
    collapsed: bool,
    hunks: Vec<HunkView>,
}

impl FileView {
    /// Where the file sits in the patch, which is what its ids are built on.
    #[must_use]
    pub fn index(&self) -> usize {
        self.index
    }

    /// The path after the change.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Lines added.
    #[must_use]
    pub fn added(&self) -> usize {
        self.added
    }

    /// Lines removed.
    #[must_use]
    pub fn removed(&self) -> usize {
        self.removed
    }

    /// How many rows the file has, which decides whether it is collapsed.
    #[must_use]
    pub fn lines(&self) -> usize {
        self.lines
    }

    /// Whether git declined to show the content.
    #[must_use]
    pub fn binary(&self) -> bool {
        self.binary
    }

    /// Whether any open note is on this file.
    #[must_use]
    pub fn noted(&self) -> bool {
        self.noted
    }

    /// Whether the table is left out of the first response and fetched on
    /// demand.
    #[must_use]
    pub fn collapsed(&self) -> bool {
        self.collapsed
    }

    /// The hunks, in order.
    #[must_use]
    pub fn hunks(&self) -> &[HunkView] {
        &self.hunks
    }

    /// Leave the table out of the page and let the reader ask for it.
    pub(crate) fn collapse(&mut self) {
        self.collapsed = true;
    }
}

/// Build one file's rows, anchoring every thread and decision that lands on
/// them as it goes.
///
/// One pass over the lines, one hash lookup per side per line. `threads` is
/// told which row it sits under and `decision_rows` learns where each
/// decision starts, so the rail can link into the diff without a second walk.
pub(crate) fn build(
    index: usize,
    file: &FileDiff,
    anchors: &Anchors,
    threads: &mut [Thread],
    decision_rows: &mut HashMap<u32, String>,
) -> FileView {
    let mut hunks = Vec::with_capacity(file.hunks().len());
    let mut position = 0_usize;
    let mut noted_file = false;

    for hunk in file.hunks() {
        let mut rows = Vec::with_capacity(hunk.lines().len());
        for line in hunk.lines() {
            let id = format!("L-{index}-{position}");
            position += 1;

            let mut row = Row {
                id,
                kind: kind_of(line.kind()),
                old: line.old_number(),
                new: line.new_number(),
                text: line.text().to_owned(),
                noted: false,
                decisions: Vec::new(),
                why: Vec::new(),
                threads: Vec::new(),
            };

            for key in keys(index, &row) {
                row.noted = row.noted || anchors.noted(key);
                for &number in anchors.decisions_at(key) {
                    row.decisions.push(number);
                    decision_rows.insert(number, row.id.clone());
                }
                row.why.extend(anchors.why_at(key).iter().cloned());
                for &thread in anchors.threads_at(key) {
                    row.threads.push(thread);
                    if let Some(found) = threads.get_mut(thread) {
                        found.anchor_to(row.id.clone());
                    }
                }
            }

            noted_file = noted_file || row.noted;
            rows.push(row);
        }
        hunks.push(HunkView {
            header: hunk.header().to_owned(),
            rows,
        });
    }

    FileView {
        index,
        path: file.path().to_owned(),
        added: file.added(),
        removed: file.removed(),
        lines: position,
        binary: file.is_binary(),
        noted: noted_file,
        collapsed: false,
        hunks,
    }
}

/// The keys a row answers to: one per side it has a number on.
fn keys(index: usize, row: &Row) -> Vec<(usize, Side, u32)> {
    let mut keys = Vec::with_capacity(2);
    if let Some(old) = row.old {
        keys.push((index, Side::Old, old));
    }
    if let Some(new) = row.new {
        keys.push((index, Side::New, new));
    }
    keys
}

/// The patch crate's word for what a line does, in ours.
fn kind_of(kind: LineKind) -> RowKind {
    match kind {
        LineKind::Context => RowKind::Context,
        LineKind::Added => RowKind::Added,
        LineKind::Removed => RowKind::Removed,
    }
}
