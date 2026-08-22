//! Opening a proposal: naming the work and writing down where it starts.

use review::{Author, Branch, Event, Proposal, ProposalId, Sha, Timestamp};

use crate::error::Result;
use crate::store::Store;

/// How much of the head commit goes in the name, so two proposals with the
/// same title never collide.
const SHORT: usize = 7;

/// How long the readable half of a name may be.
const CEILING: usize = 40;

/// Open a proposal for the work between a branch and a commit.
///
/// The base is where the two last agreed, so the diff a reviewer reads is the
/// work and not whatever else landed on the target in the meantime.
///
/// # Errors
///
/// A branch git would not accept, a revision that does not resolve, or a
/// proposal that does not move past its base.
pub fn propose(
    store: &Store,
    author: &Author,
    now: Timestamp,
    title: &str,
    onto: &str,
    revision: &str,
) -> Result<Proposal> {
    let target = Branch::parse(onto)?;
    let repo = store.repo();

    let tip = repo.head_of(&target.git_ref())?;
    let head = Sha::parse(&repo.resolve(revision)?)?;
    let base = Sha::parse(&repo.merge_base(&tip, head.as_str())?)?;

    let proposal = Proposal::open(
        slug(title, &head)?,
        title,
        target.clone(),
        base.clone(),
        head,
        now,
    )?;

    let opened = Event::opened(
        proposal.id().clone(),
        title,
        target,
        base,
        author.clone(),
        now,
    )?;

    store.open(&proposal, &opened)?;

    Ok(proposal)
}

/// Name a proposal after its title, with a piece of the commit on the end.
///
/// # Errors
///
/// Nothing a title can do: whatever is left after the substitutions is either
/// a name a ref can carry or the word `proposal`.
pub fn slug(title: &str, head: &Sha) -> Result<ProposalId> {
    let mut name = String::new();

    for letter in title.to_lowercase().chars() {
        if letter.is_ascii_lowercase() || letter.is_ascii_digit() {
            name.push(letter);
        } else if !name.ends_with('-') {
            name.push('-');
        }
    }

    // Everything left is one byte wide, so a cut at forty characters is a cut
    // at forty bytes and never lands inside one.
    let cut: String = name.trim_matches('-').chars().take(CEILING).collect();
    let short = cut.trim_matches('-');
    let readable = if short.is_empty() { "proposal" } else { short };

    Ok(ProposalId::parse(&format!(
        "{readable}-{}",
        &head.as_str()[..SHORT]
    ))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn head() -> Sha {
        Sha::parse("0123456789abcdef0123456789abcdef01234567").unwrap()
    }

    #[test]
    fn a_proposal_is_named_after_its_title() -> Result<()> {
        assert_eq!(
            slug("Land the gate, finally!", &head())?.as_str(),
            "land-the-gate-finally-0123456"
        );
        Ok(())
    }

    #[test]
    fn a_title_that_says_nothing_a_ref_can_carry_is_still_a_proposal() -> Result<()> {
        assert_eq!(slug("!!! ???", &head())?.as_str(), "proposal-0123456");
        assert_eq!(slug("", &head())?.as_str(), "proposal-0123456");
        Ok(())
    }

    #[test]
    fn a_long_title_is_cut_and_never_ends_on_a_dash() -> Result<()> {
        assert_eq!(
            slug(
                "the quick brown fox jumps over the lazy dog and keeps going",
                &head()
            )?
            .as_str(),
            "the-quick-brown-fox-jumps-over-the-lazy-0123456"
        );
        assert_eq!(
            slug("a title that runs right up to the ceiling", &head())?.as_str(),
            "a-title-that-runs-right-up-to-the-ceilin-0123456"
        );
        Ok(())
    }
}
