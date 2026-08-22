//! The conversation, arranged the way a page reads it.
//!
//! A note is a thread: the note itself, the answers under it, and whether the
//! revision it was written on is still the head. The page needs every thread
//! once, and it needs to find the threads that belong to one diff row without
//! walking the whole conversation per row, so both shapes are built here in
//! one pass over the records.

use std::collections::{HashMap, HashSet};

use review::{Proposal, RecordId, Side};

use crate::model::clock;

/// Which row of the diff something is anchored to: the file's index in the
/// patch, the side it counts on, and the line number.
pub type RowKey = (usize, Side, u32);

/// One turn in a thread: somebody saying something.
#[derive(Debug, Clone)]
pub struct Turn {
    author: String,
    body: String,
}

impl Turn {
    /// Who said it.
    #[must_use]
    pub fn author(&self) -> &str {
        &self.author
    }

    /// What they said.
    #[must_use]
    pub fn body(&self) -> &str {
        &self.body
    }
}

/// A note and everything said under it.
#[derive(Debug, Clone)]
pub struct Thread {
    id: RecordId,
    row: String,
    file: String,
    side: Side,
    line: u32,
    author: String,
    at: String,
    body: String,
    stale: Option<String>,
    resolved: bool,
    answers: Vec<Turn>,
}

impl Thread {
    /// The note's record id, which is what every button on the thread names.
    #[must_use]
    pub fn id(&self) -> &RecordId {
        &self.id
    }

    /// The id of the diff row this thread is inserted after, empty when the
    /// line it was written on is not in the diff being shown.
    #[must_use]
    pub fn row(&self) -> &str {
        &self.row
    }

    /// Whether the thread has a row to sit under.
    #[must_use]
    pub fn anchored(&self) -> bool {
        !self.row.is_empty()
    }

    /// The file the note is on.
    #[must_use]
    pub fn file(&self) -> &str {
        &self.file
    }

    /// The side of the diff the note counts its lines on.
    #[must_use]
    pub fn side(&self) -> Side {
        self.side
    }

    /// The last line of the span the note covers.
    #[must_use]
    pub fn line(&self) -> u32 {
        self.line
    }

    /// Who left the note.
    #[must_use]
    pub fn author(&self) -> &str {
        &self.author
    }

    /// When, as `HH:MM`.
    #[must_use]
    pub fn at(&self) -> &str {
        &self.at
    }

    /// What the note says.
    #[must_use]
    pub fn body(&self) -> &str {
        &self.body
    }

    /// `on r2` when the note was written against a revision that is no longer
    /// the head, nothing when it is current.
    #[must_use]
    pub fn stale(&self) -> Option<&str> {
        self.stale.as_deref()
    }

    /// Whether somebody said the note was answered.
    #[must_use]
    pub fn resolved(&self) -> bool {
        self.resolved
    }

    /// The answers, oldest first.
    #[must_use]
    pub fn answers(&self) -> &[Turn] {
        &self.answers
    }

    /// The first line of the body, which is all a list has room for.
    #[must_use]
    pub fn summary(&self) -> &str {
        self.body.lines().next().unwrap_or_default()
    }

    /// Point the thread at the row it renders under.
    pub(crate) fn anchor_to(&mut self, row: String) {
        self.row = row;
    }
}

/// Every thread on the proposal, in the order the notes were written.
///
/// Resolved notes are included: the rail still lists them under a fold, and a
/// thread that arrives over the stream has to be renderable whatever its
/// state. Only the anchors decide what appears inside the diff.
pub(crate) fn threads(proposal: &Proposal) -> Vec<Thread> {
    let head = proposal.head();
    proposal
        .comments()
        .iter()
        .map(|note| {
            let stale = (note.revision() != head.sha()).then(|| {
                match proposal.revision_of(note.revision()) {
                    Some(revision) => format!("on r{}", revision.number()),
                    None => "on an earlier revision".to_owned(),
                }
            });
            Thread {
                id: note.id().clone(),
                row: String::new(),
                file: note.anchor().file().as_str().to_owned(),
                side: note.anchor().span().side(),
                line: note.anchor().span().end(),
                author: note.author().as_str().to_owned(),
                at: clock(note.at()),
                body: note.body().to_owned(),
                stale,
                resolved: proposal.is_resolved(note.id()),
                answers: proposal
                    .answers(note.id())
                    .into_iter()
                    .map(|reply| Turn {
                        author: reply.author().as_str().to_owned(),
                        body: reply.body().to_owned(),
                    })
                    .collect(),
            }
        })
        .collect()
}

/// Everything the diff needs to look up by row, built once per page.
///
/// Every map here is filled by walking the records once; rendering then costs
/// one hash lookup per row instead of a scan of the conversation per row,
/// which is what made the old page quadratic.
#[derive(Debug, Default)]
pub(crate) struct Anchors {
    threads: HashMap<RowKey, Vec<usize>>,
    noted: HashSet<RowKey>,
    decisions: HashMap<RowKey, Vec<u32>>,
    why: HashMap<RowKey, Vec<String>>,
}

impl Anchors {
    /// Fold the records into the four lookups a diff row asks for.
    ///
    /// `index_of` maps a path in the patch to its position, so a note on a
    /// file this diff does not show is simply never anchored.
    pub(crate) fn build(
        proposal: &Proposal,
        threads: &[Thread],
        index_of: &HashMap<&str, usize>,
    ) -> Self {
        let mut anchors = Self::default();

        for (position, thread) in threads.iter().enumerate() {
            if thread.resolved() {
                continue;
            }
            if let Some(key) = key(index_of, thread.file(), thread.side(), thread.line()) {
                anchors.threads.entry(key).or_default().push(position);
            }
        }

        for note in proposal.open_comments() {
            let Some(&file) = index_of.get(note.anchor().file().as_str()) else {
                continue;
            };
            let span = note.anchor().span();
            for line in span.start()..=span.end() {
                anchors.noted.insert((file, span.side(), line));
            }
        }

        for (position, chunk) in proposal.chunks().iter().enumerate() {
            let Some(anchor) = chunk.anchor() else {
                continue;
            };
            let Some(key) = key(
                index_of,
                anchor.file().as_str(),
                anchor.span().side(),
                anchor.span().start(),
            ) else {
                continue;
            };
            let number = u32::try_from(position)
                .unwrap_or(u32::MAX)
                .saturating_add(1);
            anchors.decisions.entry(key).or_default().push(number);
        }

        for note in proposal.rationale() {
            let Some(key) = key(
                index_of,
                note.anchor().file().as_str(),
                note.anchor().span().side(),
                note.anchor().span().end(),
            ) else {
                continue;
            };
            anchors
                .why
                .entry(key)
                .or_default()
                .push(note.body().to_owned());
        }

        anchors
    }

    /// The threads that sit under that row.
    pub(crate) fn threads_at(&self, key: RowKey) -> &[usize] {
        self.threads.get(&key).map_or(&[], Vec::as_slice)
    }

    /// Whether an open note covers that row.
    pub(crate) fn noted(&self, key: RowKey) -> bool {
        self.noted.contains(&key)
    }

    /// The decisions that start on that row.
    pub(crate) fn decisions_at(&self, key: RowKey) -> &[u32] {
        self.decisions.get(&key).map_or(&[], Vec::as_slice)
    }

    /// The author's explanations that end on that row.
    pub(crate) fn why_at(&self, key: RowKey) -> &[String] {
        self.why.get(&key).map_or(&[], Vec::as_slice)
    }
}

/// The row key for a path this diff shows, and nothing for one it does not.
fn key(index_of: &HashMap<&str, usize>, file: &str, side: Side, line: u32) -> Option<RowKey> {
    index_of.get(file).map(|&index| (index, side, line))
}
