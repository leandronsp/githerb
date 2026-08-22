//! Who is working, and what time it is.
//!
//! The core takes both as parameters, which is what keeps it testable without
//! a fixture. This is where they come from, and it is the only clock in the
//! program.

use std::time::{SystemTime, UNIX_EPOCH};

use gitstore::Repo;
use review::{Author, Timestamp};

/// The environment variable that overrides everything, so a script can sign
/// the log as whoever it is acting for.
pub const AUTHOR_ENV: &str = "GITHERB_AUTHOR";

/// What a record is signed with when nobody said who is working.
pub const UNKNOWN: &str = "unknown";

/// What the runner signs with, so the log does not read as though the person
/// sitting here did the rebase.
pub const RUNNER: &str = "githerb-run";

/// Where a signature comes from.
pub struct Identity;

impl Identity {
    /// Who is working here: the environment first, then what git was told,
    /// then nobody in particular.
    #[must_use]
    pub fn detect(repo: &Repo) -> Author {
        named(std::env::var(AUTHOR_ENV).ok())
            .or_else(|| named(repo.user_name().ok().flatten()))
            .unwrap_or_else(|| literal(UNKNOWN))
    }

    /// Who the runner is, unless somebody said otherwise: a job it did is the
    /// runner's line in the log, not yours.
    #[must_use]
    pub fn runner() -> Author {
        named(std::env::var(AUTHOR_ENV).ok()).unwrap_or_else(|| literal(RUNNER))
    }
}

/// Now, to the second, which is the only resolution the wire format carries.
#[must_use]
pub fn now() -> Timestamp {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| i64::try_from(since.as_secs()).unwrap_or(0));

    Timestamp::from_unix(seconds)
}

/// A name somebody gave, when they gave one.
fn named(raw: Option<String>) -> Option<Author> {
    Author::parse(raw?.as_str()).ok()
}

/// A name this build wrote itself.
#[expect(
    clippy::expect_used,
    reason = "the constants here are non-blank, which is the only thing Author refuses"
)]
fn literal(name: &str) -> Author {
    Author::parse(name).expect("a literal author is not blank")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blank_name_is_nobody() {
        assert_eq!(named(Some("   ".to_owned())), None);
        assert_eq!(named(None), None);
        assert_eq!(
            named(Some(" ada ".to_owned())).map(|author| author.as_str().to_owned()),
            Some("ada".to_owned())
        );
    }

    #[test]
    fn nobody_in_particular_is_still_an_author() {
        assert_eq!(literal(UNKNOWN).as_str(), "unknown");
        assert_eq!(literal(RUNNER).as_str(), "githerb-run");
    }

    #[test]
    fn the_clock_moves_forward_from_the_epoch() {
        assert!(now().unix() > 1_700_000_000);
    }
}
