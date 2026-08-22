//! The core: proposals, revisions, annotations, the rules about them and the
//! wire format they are stored in. Pure functions and value objects; nothing
//! here touches a disk, a network or a clock.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod timestamp;

pub use timestamp::Timestamp;
