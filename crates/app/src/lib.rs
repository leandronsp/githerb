//! Use cases. Wires the core to the store and holds the sequence of steps a
//! command performs. One module per verb, plus what the repository declares.
//!
//! Every use case is a free function taking `&Store` and the values it needs.
//! There is no session object and no trait: the store is a concrete type, the
//! author and the clock arrive as parameters, and the whole crate is what
//! `main`, the browser and the runner all call so that the three of them
//! cannot disagree about what a verb does.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod abandon;
mod annotate;
mod check;
pub mod config;
mod describe;
mod diff;
mod dispatch;
mod error;
pub mod format;
mod handover;
mod identity;
mod land;
mod propose;
mod reply;
mod report;
mod resolve;
mod revise;
mod snapshot;
mod store;

pub use abandon::abandon;
pub use annotate::annotate;
pub use check::{check, refused, required};
pub use config::{Config, ConfigError};
pub use describe::{describe, template};
pub use diff::{diff, origin};
pub use dispatch::dispatch;
pub use error::{Error, Result};
pub use handover::{Reader, handover};
pub use identity::{Identity, now};
pub use land::{Landing, land};
pub use propose::{propose, slug};
pub use reply::reply;
pub use report::report;
pub use resolve::resolve;
pub use revise::revise;
pub use snapshot::Snapshot;
pub use store::Store;
