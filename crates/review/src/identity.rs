//! The names things go by: a commit, a record, a proposal, a person, a file.
//!
//! Each is a string with a rule, and the rule lives in the only constructor.
//! A `Sha` cannot be passed where a `FilePath` belongs, which is the whole
//! reason these exist rather than five parameters of type `String`.

use std::fmt;

use crate::errors::{Error, Result};

/// How many hex characters a commit name has.
const SHA_LENGTH: usize = 40;

/// How much of a sha is enough to recognise it.
const SHORT_LENGTH: usize = 7;

/// A full commit object name: forty lowercase hex characters.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Sha(String);

impl Sha {
    /// Read a commit name, and nothing looser.
    ///
    /// # Errors
    ///
    /// Anything that is not exactly forty lowercase hex characters.
    pub fn parse(raw: &str) -> Result<Self> {
        let hex = raw.len() == SHA_LENGTH
            && raw
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
        if hex {
            Ok(Self(raw.to_owned()))
        } else {
            Err(Error::NoRevision(raw.to_owned()))
        }
    }

    /// The forty characters.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The first seven characters, which is what a person reads.
    #[must_use]
    pub fn short(&self) -> &str {
        self.0.get(..SHORT_LENGTH).unwrap_or(&self.0)
    }
}

impl fmt::Display for Sha {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What a record is called: twelve hex characters derived from its content, so
/// the same annotation written twice is one annotation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RecordId(String);

impl RecordId {
    /// Read the identity a record carries, or that a resolution points at.
    ///
    /// # Errors
    ///
    /// A blank id, which on the wire only ever arrives as a blank target.
    pub fn parse(raw: &str) -> Result<Self> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(Error::NoTarget);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// The identity the wire layer derived from a record's own bytes. It is
    /// twelve hex characters by construction, so there is nothing to check.
    pub(crate) fn from_derived(hex: String) -> Self {
        Self(hex)
    }

    /// The twelve characters.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RecordId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What a proposal is called, and the last segment of the ref it lives in.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProposalId(String);

impl ProposalId {
    /// Read a proposal name.
    ///
    /// # Errors
    ///
    /// A blank name, or one carrying a slash or whitespace: the id is one ref
    /// segment and a slash would split it into two.
    pub fn parse(raw: &str) -> Result<Self> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(Error::NoProposalId);
        }
        if trimmed.contains('/') || trimmed.chars().any(char::is_whitespace) {
            return Err(Error::BadProposalId(raw.to_owned()));
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProposalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Who left a record: a person, an agent, or whatever ran a check.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Author(String);

impl Author {
    /// Read the name a record is signed with.
    ///
    /// # Errors
    ///
    /// A blank name. Every record says who left it.
    pub fn parse(raw: &str) -> Result<Self> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(Error::NoAuthor);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Author {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A path inside the repository, as the diff spells it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FilePath(String);

impl FilePath {
    /// Read a path.
    ///
    /// # Errors
    ///
    /// A blank path. A note points at a file or it points nowhere.
    pub fn parse(raw: &str) -> Result<Self> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(Error::NoFile);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// The path.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FilePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &str = "9f6c1e2a3b4d5e6f708192a3b4c5d6e7f8091a2b";

    // --- Sha ---

    #[test]
    fn a_sha_is_forty_lowercase_hex_characters() -> Result<()> {
        assert_eq!(Sha::parse(HEAD)?.as_str(), HEAD);
        Ok(())
    }

    #[test]
    fn a_sha_shows_its_first_seven_characters() -> Result<()> {
        assert_eq!(Sha::parse(HEAD)?.short(), "9f6c1e2");
        Ok(())
    }

    #[test]
    fn anything_that_is_not_a_commit_name_is_refused() {
        for raw in [
            "",
            "abc",
            "9F6C1E2A3B4D5E6F708192A3B4C5D6E7F8091A2B",
            "9f6c1e2a3b4d5e6f708192a3b4c5d6e7f8091a2",
            "9f6c1e2a3b4d5e6f708192a3b4c5d6e7f8091a2bb",
            "9f6c1e2a3b4d5e6f708192a3b4c5d6e7f8091a2g",
        ] {
            assert_eq!(Sha::parse(raw), Err(Error::NoRevision(raw.to_owned())));
        }
    }

    // --- RecordId ---

    #[test]
    fn a_blank_record_id_is_a_resolution_that_names_nothing() {
        assert_eq!(RecordId::parse("   "), Err(Error::NoTarget));
    }

    // --- ProposalId ---

    #[test]
    fn a_proposal_id_is_one_ref_segment() -> Result<()> {
        assert_eq!(
            ProposalId::parse(" land-the-gate ")?.as_str(),
            "land-the-gate"
        );
        assert_eq!(ProposalId::parse(""), Err(Error::NoProposalId));
        assert_eq!(
            ProposalId::parse("feat/gate"),
            Err(Error::BadProposalId("feat/gate".to_owned()))
        );
        assert_eq!(
            ProposalId::parse("land the gate"),
            Err(Error::BadProposalId("land the gate".to_owned()))
        );
        Ok(())
    }

    // --- Author and FilePath ---

    #[test]
    fn an_author_is_trimmed_and_never_blank() -> Result<()> {
        assert_eq!(Author::parse("  leandro ")?.as_str(), "leandro");
        assert_eq!(Author::parse(" \t "), Err(Error::NoAuthor));
        Ok(())
    }

    #[test]
    fn a_file_path_is_trimmed_and_never_blank() -> Result<()> {
        assert_eq!(FilePath::parse(" a.go ")?.as_str(), "a.go");
        assert_eq!(FilePath::parse(""), Err(Error::NoFile));
        Ok(())
    }
}
