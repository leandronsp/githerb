//! The command line as types: what a person may type, and nothing about what
//! it does.

use std::fmt;
use std::str::FromStr;

use clap::{Parser, Subcommand, ValueEnum};

/// What the usage text says after the commands, because none of this needs a
/// server and pushing the refs is the whole of sharing it.
const TRAILER: &str = "Everything lives in the repository: proposals are refs under \
refs/githerb/proposals, annotations are notes. Nothing here needs a server, and pushing those \
refs is how a colleague sees them.";

/// githerb proposes work, collects annotations on it and lands it.
#[derive(Debug, Parser)]
#[command(name = "githerb", version, about, after_help = TRAILER)]
pub struct Cli {
    /// What to do.
    #[command(subcommand)]
    pub command: Command,
}

/// Every verb the terminal offers.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Open a proposal for the work between a branch and a commit
    Propose {
        /// The commit to propose, anything git resolves
        #[arg(default_value = "HEAD")]
        revision: String,
        /// The branch this lands on
        #[arg(long, default_value = "main")]
        onto: String,
        /// What the proposal does
        #[arg(long)]
        title: String,
    },
    /// Every proposal, one row each
    List,
    /// One proposal: where it goes, what ran on it, what is open
    Show {
        /// Which proposal
        proposal: String,
    },
    /// The patch a reviewer reads
    Diff {
        /// Which proposal
        proposal: String,
        /// Start from this revision instead of the base
        #[arg(long)]
        since: Option<u32>,
    },
    /// Leave a note on a range of lines
    Comment {
        /// Which proposal
        proposal: String,
        /// The file the note is about
        #[arg(long)]
        file: String,
        /// A line, or a range as N:M
        #[arg(long)]
        line: Lines,
        /// Which side of the diff
        #[arg(long, default_value = "new")]
        side: String,
        /// What the note says
        #[arg(long)]
        body: String,
    },
    /// The notes on a proposal
    Comments {
        /// Which proposal
        proposal: String,
        /// One record per line, the wire bytes, for an agent to read
        #[arg(long)]
        json: bool,
        /// Every note nobody resolved, not only the ones on the head
        #[arg(long)]
        all: bool,
    },
    /// Answer a note, in words
    Reply {
        /// Which proposal
        proposal: String,
        /// Which note
        comment: String,
        /// What the answer says
        #[arg(long)]
        body: String,
    },
    /// Say a note is dealt with
    Resolve {
        /// Which proposal
        proposal: String,
        /// Which note
        comment: String,
    },
    /// The open notes as one brief, for an agent
    Handover {
        /// Which proposal
        proposal: String,
        /// What a runner hands an agent, decisions and all
        #[arg(long)]
        agent: bool,
    },
    /// Write down what an agent is doing
    Work {
        /// Whether it started, finished or gave up
        phase: Phase,
        /// Which proposal
        proposal: String,
        /// apply, rebase or check
        #[arg(long)]
        task: String,
        /// One line about it, usually why it stopped
        #[arg(long)]
        note: Option<String>,
    },
    /// Hand the open notes to an agent
    Dispatch {
        /// Which proposal
        proposal: String,
    },
    /// Record another attempt at a proposal
    Revise {
        /// Which proposal
        proposal: String,
        /// The commit to record, anything git resolves
        #[arg(default_value = "HEAD")]
        revision: String,
    },
    /// The decisions a proposal carries, read as JSON on stdin
    Describe {
        /// Which proposal
        #[arg(required_unless_present = "template")]
        proposal: Option<String>,
        /// Print the shape a description takes
        #[arg(long)]
        template: bool,
    },
    /// Run what the repository declares against the head revision
    Check {
        /// Which proposal
        proposal: String,
    },
    /// Move the target branch onto the head revision
    Land {
        /// Which proposal
        proposal: String,
    },
    /// Say a proposal will not be landing
    Abandon {
        /// Which proposal
        proposal: String,
    },
    /// Answer the log on its own, for a machine that serves no pages
    Run {
        /// Take one pass and stop
        #[arg(long)]
        once: bool,
        /// How long to wait between passes when nothing moves, e.g. 2s, 500ms, 1m
        #[arg(long, default_value = "2s")]
        every: Every,
    },
    /// What this build is called
    Version,
}

/// A pause a person types: `2s`, `500ms`, `1m`, or a bare number of seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Every(pub std::time::Duration);

/// Why a pause was not a pause.
#[derive(Debug, PartialEq, Eq)]
pub struct NotADuration(String);

impl fmt::Display for NotADuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} is not a duration like 2s, 500ms or 1m", self.0)
    }
}

impl std::error::Error for NotADuration {}

impl FromStr for Every {
    type Err = NotADuration;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let refuse = || NotADuration(format!("{raw:?}"));
        let text = raw.trim();
        let (digits, unit) = match text.find(|c: char| !c.is_ascii_digit()) {
            Some(at) => text.split_at(at),
            None => (text, "s"),
        };
        let amount: u64 = digits.parse().map_err(|_ignored| refuse())?;
        let millis = match unit.trim() {
            "ms" => amount,
            "s" => amount * 1000,
            "m" => amount * 60_000,
            _ => return Err(refuse()),
        };
        if millis == 0 {
            return Err(refuse());
        }
        Ok(Every(std::time::Duration::from_millis(millis)))
    }
}

impl fmt::Display for Every {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let millis = self.0.as_millis();
        if millis.is_multiple_of(60_000) {
            write!(f, "{}m", millis / 60_000)
        } else if millis.is_multiple_of(1000) {
            write!(f, "{}s", millis / 1000)
        } else {
            write!(f, "{millis}ms")
        }
    }
}

/// The phase a person types, which is not quite the word the log keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Phase {
    /// An agent picked the work up.
    Start,
    /// It finished.
    Done,
    /// It gave up, and says why.
    Fail,
}

impl Phase {
    /// The word the record carries.
    pub fn recorded(self) -> &'static str {
        match self {
            Phase::Start => "started",
            Phase::Done => "finished",
            Phase::Fail => "failed",
        }
    }
}

/// One line, or a range of them, as `N` or `N:M`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lines {
    /// The first line.
    pub start: u32,
    /// The last one, which is the first one when nobody said otherwise.
    pub end: u32,
}

/// Why a line was not a line.
#[derive(Debug, PartialEq, Eq)]
pub struct NotALine(String);

impl fmt::Display for NotALine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} is not a line", self.0)
    }
}

impl std::error::Error for NotALine {}

impl FromStr for Lines {
    type Err = NotALine;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let refuse = || NotALine(format!("{raw:?}"));
        let (first, last) = raw.trim().split_once(':').unwrap_or((raw.trim(), ""));

        let start = first.trim().parse::<u32>().map_err(|_ignored| refuse())?;
        let end = if last.trim().is_empty() {
            start
        } else {
            last.trim().parse::<u32>().map_err(|_ignored| refuse())?
        };

        Ok(Lines { start, end })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_is_a_number_or_a_range() {
        assert_eq!("12".parse(), Ok(Lines { start: 12, end: 12 }));
        assert_eq!(" 12:18 ".parse(), Ok(Lines { start: 12, end: 18 }));
    }

    #[test]
    fn anything_else_is_not_a_line() {
        assert!("".parse::<Lines>().is_err());
        assert!("a".parse::<Lines>().is_err());
        assert!("12:x".parse::<Lines>().is_err());
        assert_eq!(
            "12:x".parse::<Lines>().unwrap_err().to_string(),
            "\"12:x\" is not a line"
        );
    }

    #[test]
    fn a_pause_is_seconds_millis_or_minutes() {
        assert_eq!("2s".parse(), Ok(Every(std::time::Duration::from_secs(2))));
        assert_eq!(
            "500ms".parse(),
            Ok(Every(std::time::Duration::from_millis(500)))
        );
        assert_eq!("1m".parse(), Ok(Every(std::time::Duration::from_secs(60))));
        assert_eq!("3".parse(), Ok(Every(std::time::Duration::from_secs(3))));
        assert_eq!(
            Every(std::time::Duration::from_millis(1500)).to_string(),
            "1500ms"
        );
        assert_eq!(Every(std::time::Duration::from_secs(120)).to_string(), "2m");
    }

    #[test]
    fn anything_else_is_not_a_pause() {
        for raw in ["", "0s", "2h", "abc", "-1s"] {
            assert!(raw.parse::<Every>().is_err(), "{raw:?}");
        }
    }

    #[test]
    fn a_phase_a_person_types_is_a_phase_the_log_keeps() {
        assert_eq!(Phase::Start.recorded(), "started");
        assert_eq!(Phase::Done.recorded(), "finished");
        assert_eq!(Phase::Fail.recorded(), "failed");
    }
}
