//! The local review surface: an HTTP/1.1 server on `std::net` and nothing else.
//!
//! It is hand-rolled on purpose. The whole server is a listener, a thread per
//! connection, one request each and one long-lived event stream per open page,
//! which is small enough to read in an afternoon and has no runtime, no
//! dependency and no configuration behind it.
//!
//! Three pieces:
//!
//! - [`Server`] accepts connections and calls a [`Handler`], which answers with
//!   a [`Reply`]: one [`Response`], or a stream fed through a [`Sink`].
//! - [`Sink`] writes server-sent events, the only way the page is pushed to.
//! - [`Watcher`] probes one value on one thread and wakes every
//!   [`Subscription`] when it moves, so twenty tabs cost one probe.
//!
//! On top of that sit the page and the markup: [`Page`] precomputes one render
//! of a proposal and [`render`] writes it out, so no template ever asks a
//! question that costs a scan.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod assets;
mod error;
#[cfg(test)]
pub mod fixtures;
mod form;
mod http;
pub mod model;
pub mod render;
mod request;
mod response;
mod sse;
mod watch;

pub use assets::{asset_hash, static_asset};
pub use error::Error;
pub use http::{Handler, Reply, Server};
pub use model::{Board, Page, fingerprint};
pub use request::{MAX_BODY, Method, Request};
pub use response::Response;
pub use sse::Sink;
pub use watch::{Subscription, Wakeup, Watcher};
