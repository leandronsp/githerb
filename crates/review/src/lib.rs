//! The core: proposals, revisions, annotations, the rules about them and the
//! wire format they are stored in. Pure functions and value objects; nothing
//! here touches a disk, a network or a clock.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

#[cfg(test)]
pub mod fixtures;

pub mod branch;
pub mod check;
pub mod chunk;
pub mod comment;
pub mod errors;
pub mod handover;
pub mod identity;
pub mod lifecycle;
pub mod proposal;
pub mod rationale;
pub mod record;
pub mod reply;
pub mod resolution;
pub mod span;
pub mod timestamp;
pub mod wire;
pub mod work;

pub use branch::Branch;
pub use check::{Check, CheckName, CheckStatus};
pub use chunk::{Chunk, Field};
pub use comment::Comment;
pub use errors::{Error, Result};
pub use handover::{brief, conflict_brief, handover};
pub use identity::{Author, FilePath, ProposalId, RecordId, Sha};
pub use lifecycle::{Event, EventKind};
pub use proposal::views::CheckSummary;
pub use proposal::{Proposal, Revision, State};
pub use rationale::Rationale;
pub use record::{Kind, Record};
pub use reply::Reply;
pub use resolution::Resolution;
pub use span::{Anchor, Side, Span};
pub use timestamp::Timestamp;
pub use wire::derive_id;
pub use work::{Activity, Dispatch, Phase, Task, Work, activity};
