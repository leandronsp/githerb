//! Another attempt at a proposal, which is what an agent leaves behind after
//! reading the annotations on the last one.

use review::{Proposal, ProposalId, Sha};

use crate::error::Result;
use crate::store::Store;

/// Record the commit as the next revision of the proposal.
///
/// # Errors
///
/// A proposal nobody opened, a revision that does not resolve, a proposal
/// that is no longer open, or a revision it already carries.
pub fn revise(store: &Store, id: &ProposalId, revision: &str) -> Result<Proposal> {
    let mut proposal = store.load(id)?;
    let sha = Sha::parse(&store.repo().resolve(revision)?)?;

    proposal.add_revision(sha)?;

    let head = proposal.head();
    store.revise(id, head.number(), head.sha())?;

    Ok(proposal)
}
