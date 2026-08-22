//! What the bar says about the proposal: the checks, who is working on it and
//! where the diff is measured from.
//!
//! Every value here is derived from the records. None of it is stored, and
//! none of it is a field anybody writes twice.

use review::{CheckName, Proposal, Revision};

/// What a required check has to say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckState {
    /// It ran and it agreed.
    Passed,
    /// It ran and it said no.
    Failed,
    /// The repository requires it and nothing has run it on this revision.
    Missing,
}

impl CheckState {
    /// The class the chip carries.
    #[must_use]
    pub fn class(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Missing => "missing",
        }
    }
}

/// One check on the head revision.
#[derive(Debug, Clone)]
pub struct CheckRow {
    name: String,
    state: CheckState,
    seconds: u32,
}

impl CheckRow {
    /// What the check is called.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// How it went.
    #[must_use]
    pub fn state(&self) -> CheckState {
        self.state
    }

    /// How long it took, zero when it has not run.
    #[must_use]
    pub fn seconds(&self) -> u32 {
        self.seconds
    }
}

/// The checks on the head revision, with the required ones that have not run.
///
/// Sorted by name so the strip does not reorder itself between renders.
pub(crate) fn checks(proposal: &Proposal, required: &[CheckName]) -> Vec<CheckRow> {
    let ran = proposal.checks();
    let mut rows: Vec<CheckRow> = ran
        .iter()
        .map(|check| CheckRow {
            name: check.name().as_str().to_owned(),
            state: if check.passed() {
                CheckState::Passed
            } else {
                CheckState::Failed
            },
            seconds: check.seconds(),
        })
        .collect();

    for name in required {
        if ran.iter().any(|check| check.name() == name) {
            continue;
        }
        rows.push(CheckRow {
            name: name.as_str().to_owned(),
            state: CheckState::Missing,
            seconds: 0,
        });
    }

    rows.sort_by(|left, right| left.name.cmp(&right.name));
    rows
}

/// Whether an agent is on the proposal, and what that looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentState {
    /// An agent claimed the head revision and has not come back.
    Working,
    /// The last thing an agent did on this revision failed.
    Failed,
    /// Somebody handed the notes over and nothing has picked them up.
    Waiting,
    /// Nobody is on it.
    Idle,
}

impl AgentState {
    /// The class the chip carries.
    #[must_use]
    pub fn class(self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::Failed => "failed",
            Self::Waiting => "waiting",
            Self::Idle => "idle",
        }
    }
}

/// The one sentence the bar says about who is working.
#[derive(Debug, Clone)]
pub struct Agent {
    state: AgentState,
    text: String,
    note: Option<String>,
}

impl Agent {
    /// How to draw it.
    #[must_use]
    pub fn state(&self) -> AgentState {
        self.state
    }

    /// What it says.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Whatever the agent left behind, which the chip carries as its title.
    #[must_use]
    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }
}

/// Fold the work log on the head revision into the chip.
pub(crate) fn agent(proposal: &Proposal) -> Agent {
    let activity = proposal.activity();
    let state = match &activity {
        Some(activity) if activity.working() => AgentState::Working,
        Some(activity) if activity.failed() => AgentState::Failed,
        Some(_) | None => {
            if proposal.dispatched() {
                AgentState::Waiting
            } else {
                AgentState::Idle
            }
        }
    };
    Agent {
        state,
        text: proposal.agent_line(),
        note: activity.and_then(|activity| activity.note().map(str::to_owned)),
    }
}

/// One end of the revision strip: where the diff can be measured from.
#[derive(Debug, Clone)]
pub struct Origin {
    label: String,
    since: u32,
    active: bool,
}

impl Origin {
    /// `base`, or `r2` for a revision.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The `since` value that selects it; zero is the base.
    #[must_use]
    pub fn since(&self) -> u32 {
        self.since
    }

    /// Whether the diff on the page is measured from here.
    #[must_use]
    pub fn active(&self) -> bool {
        self.active
    }
}

/// The base and every revision but the head, which is the fixed other end.
pub(crate) fn origins(proposal: &Proposal, since: Option<u32>) -> Vec<Origin> {
    let head = proposal.head().number();
    let chosen = since.unwrap_or(0);
    let mut strip = vec![Origin {
        label: "base".to_owned(),
        since: 0,
        active: chosen == 0 || chosen >= head,
    }];
    strip.extend(
        proposal
            .revisions()
            .into_iter()
            .map(Revision::number)
            .filter(|number| *number < head)
            .map(|number| Origin {
                label: format!("r{number}"),
                since: number,
                active: chosen == number,
            }),
    );
    strip
}
