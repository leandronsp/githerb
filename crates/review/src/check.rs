//! What a command said about one revision.
//!
//! Who ran it is a field, not an architecture: the same record comes from a
//! laptop, from a loop on a spare machine, or from whatever CI the project
//! already pays for.

use std::fmt;

use crate::errors::{Error, Result};
use crate::identity::{Author, Sha};
use crate::timestamp::Timestamp;

/// What a check is called in the repository's configuration.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckName(String);

impl CheckName {
    /// Read a check name.
    ///
    /// # Errors
    ///
    /// A blank name.
    pub fn parse(raw: &str) -> Result<Self> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(Error::NoCheckName);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CheckName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// How a check ended. There is no third answer worth recording, because a
/// check that did not finish tells the gate nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CheckStatus {
    /// It said yes.
    Passed,
    /// It said no.
    Failed,
}

impl CheckStatus {
    /// Read a status off the wire.
    ///
    /// # Errors
    ///
    /// Anything that is not `passed` or `failed`.
    pub fn parse(raw: &str) -> Result<Self> {
        match raw {
            "passed" => Ok(Self::Passed),
            "failed" => Ok(Self::Failed),
            _ => Err(Error::UnknownStatus(raw.to_owned())),
        }
    }

    /// The word the wire format uses.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
        }
    }
}

impl fmt::Display for CheckStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One result, against one revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    name: CheckName,
    status: CheckStatus,
    revision: Sha,
    seconds: u32,
    author: Author,
    at: Timestamp,
}

impl Check {
    /// The only way to build one. Nothing here can be refused: every field
    /// arrives already validated.
    #[must_use]
    pub fn new(
        name: CheckName,
        status: CheckStatus,
        revision: Sha,
        seconds: u32,
        author: Author,
        at: Timestamp,
    ) -> Self {
        Self {
            name,
            status,
            revision,
            seconds,
            author,
            at,
        }
    }

    /// What the check is called.
    #[must_use]
    pub fn name(&self) -> &CheckName {
        &self.name
    }

    /// How it ended.
    #[must_use]
    pub fn status(&self) -> CheckStatus {
        self.status
    }

    /// The commit it ran against.
    #[must_use]
    pub fn revision(&self) -> &Sha {
        &self.revision
    }

    /// How long it took.
    #[must_use]
    pub fn seconds(&self) -> u32 {
        self.seconds
    }

    /// Who or what ran it.
    #[must_use]
    pub fn author(&self) -> &Author {
        &self.author
    }

    /// When, to the second, in UTC.
    #[must_use]
    pub fn at(&self) -> Timestamp {
        self.at
    }

    /// Whether it said yes.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.status == CheckStatus::Passed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &str = "9f6c1e2a3b4d5e6f708192a3b4c5d6e7f8091a2b";

    #[test]
    fn a_check_either_passed_or_failed() -> Result<()> {
        assert_eq!(CheckStatus::parse("passed")?, CheckStatus::Passed);
        assert_eq!(CheckStatus::parse("failed")?, CheckStatus::Failed);
        for raw in ["", "flaky", "PASSED"] {
            assert_eq!(
                CheckStatus::parse(raw),
                Err(Error::UnknownStatus(raw.to_owned()))
            );
        }
        Ok(())
    }

    #[test]
    fn a_check_with_no_name_is_refused() {
        assert_eq!(CheckName::parse("  "), Err(Error::NoCheckName));
    }

    #[test]
    fn a_check_knows_whether_it_opened_the_gate() -> Result<()> {
        let check = Check::new(
            CheckName::parse("suite")?,
            CheckStatus::Failed,
            Sha::parse(HEAD)?,
            41,
            Author::parse("githerb-ci@laptop")?,
            Timestamp::from_unix(1_787_335_445),
        );
        assert!(!check.passed());
        assert_eq!(check.seconds(), 41);
        Ok(())
    }
}
