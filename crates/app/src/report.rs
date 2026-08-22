//! One line of what an agent is doing. It is the only way anything gets into
//! the work log, and the log is the only thing that says whether somebody is
//! already on this.

use review::{Author, Phase, ProposalId, Record, Task, Timestamp, Work};

use crate::error::Result;
use crate::store::Store;

/// Write down that a task started, finished or failed on the head revision.
///
/// # Errors
///
/// A proposal nobody opened, a task or a phase this build does not know, or
/// a note carrying more than one line.
pub fn report(
    store: &Store,
    author: &Author,
    now: Timestamp,
    id: &ProposalId,
    task: &str,
    phase: &str,
    note: Option<&str>,
) -> Result<Work> {
    let mut proposal = store.load(id)?;
    let head = proposal.head().sha().clone();

    let line = Work::new(
        head.clone(),
        Task::parse(task)?,
        Phase::parse(phase)?,
        author.clone(),
        note,
        now,
    )?;
    let record = Record::Work(line.clone());

    proposal.apply(record.clone())?;
    store.annotate(&head, &record)?;

    Ok(line)
}
