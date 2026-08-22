//! Use cases. Wires the core to the store and holds the sequence of steps a
//! command performs. One module per verb, plus what the repository declares.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod config;

pub use config::{Config, ConfigError};
