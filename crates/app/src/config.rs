//! What the repository declares: its checks and its agent.
//!
//! `.githerb.toml` at the repository root. No file is not an error, because a
//! repository that declares nothing still has proposals to review; a file that
//! does not parse is, because silence would be mistaken for "no checks".

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use serde::Deserialize;

/// The file name, relative to the repository root.
pub const FILE: &str = ".githerb.toml";

/// The declared checks and agent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    checks: BTreeMap<String, String>,
    agent: Option<String>,
}

/// Why the configuration could not be read.
#[derive(Debug)]
pub enum ConfigError {
    /// The file exists and could not be read.
    Read(std::io::Error),
    /// The file is not the TOML this build expects.
    Bad(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(err) => write!(f, "reading {FILE}: {err}"),
            Self::Bad(err) => write!(f, "{FILE}: bad configuration: {err}"),
        }
    }
}

impl std::error::Error for ConfigError {}

#[derive(Deserialize, Default)]
struct Raw {
    #[serde(default)]
    checks: BTreeMap<String, String>,
    #[serde(default)]
    agent: RawAgent,
}

#[derive(Deserialize, Default)]
struct RawAgent {
    #[serde(default)]
    command: String,
}

impl Config {
    /// Read `.githerb.toml` under `root`. A missing file is the empty configuration.
    ///
    /// # Errors
    ///
    /// A file that exists but cannot be read, or does not parse.
    pub fn load(root: &Path) -> Result<Self, ConfigError> {
        match std::fs::read_to_string(root.join(FILE)) {
            Ok(text) => Self::parse(&text),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(ConfigError::Read(err)),
        }
    }

    /// Read the configuration from TOML text.
    ///
    /// # Errors
    ///
    /// Text that is not the TOML this build expects.
    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        let raw: Raw = toml::from_str(text).map_err(|err| ConfigError::Bad(err.to_string()))?;
        let agent = Some(raw.agent.command.trim().to_owned()).filter(|command| !command.is_empty());
        Ok(Self {
            checks: raw.checks,
            agent,
        })
    }

    /// The checks a proposal has to pass, by name, sorted ascending.
    #[must_use]
    pub fn checks(&self) -> &BTreeMap<String, String> {
        &self.checks
    }

    /// The command of the check with that name.
    #[must_use]
    pub fn check(&self, name: &str) -> Option<&str> {
        self.checks.get(name).map(String::as_str)
    }

    /// The names of the required checks, sorted ascending.
    #[must_use]
    pub fn required(&self) -> Vec<&str> {
        self.checks.keys().map(String::as_str).collect()
    }

    /// The shell command that answers a handover, if the repository declares one.
    #[must_use]
    pub fn agent(&self) -> Option<&str> {
        self.agent.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_checks_and_sorts_the_required_names() -> Result<(), ConfigError> {
        let config = Config::parse("[checks]\nsuite = \"make test\"\nlint = \"make lint\"\n")?;
        assert_eq!(config.required(), vec!["lint", "suite"]);
        assert_eq!(config.check("suite"), Some("make test"));
        Ok(())
    }

    #[test]
    fn reads_the_agent_command() -> Result<(), ConfigError> {
        let config = Config::parse("[agent]\ncommand = \"claude -p\"\n")?;
        assert_eq!(config.agent(), Some("claude -p"));
        Ok(())
    }

    #[test]
    fn a_blank_agent_command_is_no_agent() -> Result<(), ConfigError> {
        assert_eq!(Config::parse("[agent]\ncommand = \"  \"\n")?.agent(), None);
        assert_eq!(Config::parse("")?.agent(), None);
        Ok(())
    }

    #[test]
    fn no_file_is_the_empty_configuration() -> Result<(), ConfigError> {
        let config = Config::load(Path::new("/nonexistent/githerb-config-test"))?;
        assert_eq!(config, Config::default());
        assert!(config.required().is_empty());
        Ok(())
    }

    #[test]
    fn a_file_that_does_not_parse_is_refused() {
        let err = Config::parse("[checks\nbroken").unwrap_err();
        assert!(matches!(err, ConfigError::Bad(_)), "{err}");
        assert!(
            err.to_string()
                .starts_with(".githerb.toml: bad configuration")
        );
    }
}
