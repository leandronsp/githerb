//! The wire format: one line of JSON per record, and the identity derived from
//! it.
//!
//! This is a contract with other people's tooling. An agent reads a line
//! without asking us anything, so the field names, the field order and the
//! rules about which fields are omitted are all load-bearing, and the format
//! carries a version for the day one of them has to change.

mod annotation;
mod event;
mod line;

pub use line::derive_id;

pub(crate) use line::{note_id, reply_id, resolution_id};
