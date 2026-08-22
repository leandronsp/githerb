//! The patch a reviewer reads: from where the work was cut to where it is
//! now, or from any revision along the way.

use review::{Proposal, ProposalId, Sha};

use crate::error::{Error, Result};
use crate::store::Store;

/// The diff of a proposal, from its base or from one of its revisions.
///
/// # Errors
///
/// A proposal nobody opened, a revision number it does not carry, or git
/// refusing to diff.
pub fn diff(store: &Store, id: &ProposalId, since: Option<u32>) -> Result<String> {
    let proposal = store.load(id)?;
    let from = origin(&proposal, since)?;

    Ok(store
        .repo()
        .diff(from.as_str(), proposal.head().sha().as_str())?)
}

/// Where the diff starts: the base, or the revision the reader asked for.
///
/// Reading from a revision is how you see what changed since you last looked,
/// which is a different question from what the proposal does.
///
/// # Errors
///
/// A revision number this proposal does not carry.
pub fn origin(proposal: &Proposal, since: Option<u32>) -> Result<Sha> {
    let Some(number) = since else {
        return Ok(proposal.base().clone());
    };

    proposal
        .revisions()
        .into_iter()
        .find(|revision| revision.number() == number)
        .map(|revision| revision.sha().clone())
        .ok_or(Error::NoSuchRevision(number))
}

#[cfg(test)]
mod tests {
    use review::{Branch, Timestamp};

    use super::*;

    fn sha(letter: char) -> Sha {
        Sha::parse(&std::iter::repeat_n(letter, 40).collect::<String>()).unwrap()
    }

    fn proposal() -> Proposal {
        let mut proposal = Proposal::open(
            ProposalId::parse("gate-0123456").unwrap(),
            "the gate",
            Branch::parse("main").unwrap(),
            sha('b'),
            sha('a'),
            Timestamp::from_unix(1_760_000_000),
        )
        .unwrap();
        proposal.add_revision(sha('c')).unwrap();
        proposal
    }

    #[test]
    fn a_diff_starts_at_the_base_unless_a_revision_is_named() -> Result<()> {
        assert_eq!(origin(&proposal(), None)?, sha('b'));
        assert_eq!(origin(&proposal(), Some(1))?, sha('a'));
        assert_eq!(origin(&proposal(), Some(2))?, sha('c'));
        Ok(())
    }

    #[test]
    fn a_revision_this_proposal_never_had_is_refused() {
        let err = origin(&proposal(), Some(3)).unwrap_err();

        assert!(matches!(err, Error::NoSuchRevision(3)), "{err}");
    }
}
