//! The left rail: what this change decides, who has been working, and every
//! note in one list.
//!
//! The rail is the part of the page that answers "what am I looking at" before
//! any code is read, so it is built from the records the same way the diff is:
//! once, in order, with nothing derived twice.

use review::{Proposal, Work};

use crate::model::clock;

/// How many lines of the work log the rail has room for.
const TIMELINE_DEPTH: usize = 8;

/// One reviewable decision the author explained.
#[derive(Debug, Clone)]
pub struct Decision {
    number: u32,
    title: String,
    surface: Option<String>,
    before: String,
    after: String,
    call: String,
    rejected: Option<String>,
    row: Option<String>,
    at: Option<String>,
}

impl Decision {
    /// Its position in the list, which is also what the diff chips carry.
    #[must_use]
    pub fn number(&self) -> u32 {
        self.number
    }

    /// What the decision is called.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Where in the system it applies, dropped when it only repeats the file.
    #[must_use]
    pub fn surface(&self) -> Option<&str> {
        self.surface.as_deref()
    }

    /// How it was.
    #[must_use]
    pub fn before(&self) -> &str {
        &self.before
    }

    /// How it is.
    #[must_use]
    pub fn after(&self) -> &str {
        &self.after
    }

    /// The call that was made.
    #[must_use]
    pub fn call(&self) -> &str {
        &self.call
    }

    /// What was considered and turned down.
    #[must_use]
    pub fn rejected(&self) -> Option<&str> {
        self.rejected.as_deref()
    }

    /// The diff row it starts on, when the diff shows it.
    #[must_use]
    pub fn row(&self) -> Option<&str> {
        self.row.as_deref()
    }

    /// `file:start` or `file:start-end`, when it is anchored.
    #[must_use]
    pub fn at(&self) -> Option<&str> {
        self.at.as_deref()
    }
}

/// The decisions in the order they were written, which is the order they are
/// meant to be read in.
///
/// `rows` says where each decision starts in the rendered diff; a decision
/// anchored to a file this diff does not show simply has no link.
pub(crate) fn decisions(
    proposal: &Proposal,
    rows: &std::collections::HashMap<u32, String>,
) -> Vec<Decision> {
    proposal
        .chunks()
        .iter()
        .enumerate()
        .map(|(position, chunk)| {
            let number = u32::try_from(position)
                .unwrap_or(u32::MAX)
                .saturating_add(1);
            let at = chunk.anchor().map(|anchor| {
                let span = anchor.span();
                if span.start() == span.end() {
                    format!("{}:{}", anchor.file(), span.start())
                } else {
                    format!("{}:{}-{}", anchor.file(), span.start(), span.end())
                }
            });
            Decision {
                number,
                title: chunk.title().to_owned(),
                surface: surface(chunk),
                before: chunk.before().to_owned(),
                after: chunk.after().to_owned(),
                call: chunk.decision().to_owned(),
                rejected: chunk.rejected().map(str::to_owned),
                row: rows.get(&number).cloned(),
                at,
            }
        })
        .collect()
}

/// The surface, unless it only repeats the file the decision is anchored to.
fn surface(chunk: &review::Chunk) -> Option<String> {
    let surface = chunk.surface()?;
    match chunk.anchor() {
        Some(anchor) if anchor.file().as_str().starts_with(surface) => None,
        Some(_) | None => Some(surface.to_owned()),
    }
}

/// One line of the work log.
#[derive(Debug, Clone)]
pub struct Entry {
    at: String,
    agent: String,
    task: String,
    phase: String,
    note: Option<String>,
}

impl Entry {
    /// When, as `HH:MM`.
    #[must_use]
    pub fn at(&self) -> &str {
        &self.at
    }

    /// Who did it.
    #[must_use]
    pub fn agent(&self) -> &str {
        &self.agent
    }

    /// What was being done.
    #[must_use]
    pub fn task(&self) -> &str {
        &self.task
    }

    /// How far it got, which is also the class the line carries.
    #[must_use]
    pub fn phase(&self) -> &str {
        &self.phase
    }

    /// The one line the agent left behind.
    #[must_use]
    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }
}

/// The last few things an agent did, newest first.
pub(crate) fn timeline(proposal: &Proposal) -> Vec<Entry> {
    let mut work: Vec<&Work> = proposal.work().iter().collect();
    work.sort_by_key(|line| std::cmp::Reverse(line.at()));
    work.into_iter()
        .take(TIMELINE_DEPTH)
        .map(|line| Entry {
            at: clock(line.at()),
            agent: line.agent().as_str().to_owned(),
            task: line.task().as_str().to_owned(),
            phase: line.phase().as_str().to_owned(),
            note: line.note().map(str::to_owned),
        })
        .collect()
}
