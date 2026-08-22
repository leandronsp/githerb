//! Where a proposal lands. Usually the trunk, and nothing here cares.
//!
//! Landing onto another branch is how a stack is built before any of it
//! reaches main, so the rules are git's own, kept to the ones that matter: no
//! leading dash so a name can never be read as a flag, no path tricks, no ref
//! syntax.

use std::fmt;

use crate::errors::{Error, Result};

/// The suffix git reserves for its own lock files, matched exactly as git
/// matches it.
const LOCK: &str = ".lock";

/// The branch a proposal is meant to land on.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Branch(String);

impl Branch {
    /// Read a branch name, and be the only door into one.
    ///
    /// # Errors
    ///
    /// A blank name, or one git would refuse: a leading `-` or `/`, a trailing
    /// `/` or `.lock`, `..` or `//` anywhere, or any of `` ~^:?*[\ `` or a space.
    pub fn parse(raw: &str) -> Result<Self> {
        let name = raw.trim();
        let bad = || Error::BadBranch(raw.to_owned());
        if name.is_empty() {
            return Err(Error::NoBranch);
        }
        if name.starts_with('-') || name.starts_with('/') || name.ends_with('/') {
            return Err(bad());
        }
        if name.contains("..") || name.contains("//") || name.ends_with(LOCK) {
            return Err(bad());
        }
        if name.contains([' ', '~', '^', ':', '?', '*', '[', '\\']) {
            return Err(bad());
        }
        Ok(Self(name.to_owned()))
    }

    /// The name on its own.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The fully qualified ref the branch lives in.
    #[must_use]
    pub fn git_ref(&self) -> String {
        format!("refs/heads/{}", self.0)
    }
}

impl fmt::Display for Branch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_names_git_accepts_are_accepted() -> Result<()> {
        for raw in ["main", "trunk", "feature/gate", "release-2.1", "a"] {
            assert_eq!(Branch::parse(raw)?.as_str(), raw);
        }
        Ok(())
    }

    #[test]
    fn a_branch_knows_the_ref_it_lives_in() -> Result<()> {
        assert_eq!(
            Branch::parse("feature/gate")?.git_ref(),
            "refs/heads/feature/gate"
        );
        Ok(())
    }

    #[test]
    fn a_blank_branch_is_a_proposal_that_lands_nowhere() {
        assert_eq!(Branch::parse("   "), Err(Error::NoBranch));
    }

    #[test]
    fn the_names_git_would_refuse_are_refused() {
        for raw in [
            "-force",
            "/main",
            "main/",
            "a..b",
            "a//b",
            "main.lock",
            "my branch",
            "main^",
            "a:b",
            "a~1",
            "a?b",
            "a*b",
            "a[b",
            "a\\b",
        ] {
            assert_eq!(Branch::parse(raw), Err(Error::BadBranch(raw.to_owned())));
        }
    }
}
