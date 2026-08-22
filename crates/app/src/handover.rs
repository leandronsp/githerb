//! The whole review as one instruction, in the words its reader needs.

use review::ProposalId;

use crate::error::Result;
use crate::store::Store;

/// Who is about to read the brief.
///
/// A person runs `githerb resolve` and `githerb revise` themselves; a runner
/// records the revision itself and an agent told to record it too records it
/// first, which leaves the runner looking like the one that failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reader {
    /// A person, reading it on a terminal or copying it out of the browser.
    Person,
    /// An agent, handed it on stdin by a runner.
    Agent,
}

/// The open notes as one brief. Empty when nothing is open, because there is
/// nothing to say.
///
/// # Errors
///
/// A proposal nobody opened.
pub fn handover(store: &Store, id: &ProposalId, reader: Reader) -> Result<String> {
    let proposal = store.load(id)?;

    Ok(match reader {
        Reader::Person => review::handover(&proposal),
        Reader::Agent => review::brief(&proposal),
    })
}
