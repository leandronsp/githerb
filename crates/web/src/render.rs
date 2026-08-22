//! The markup. One document per page, and fragments the event stream pushes.
//!
//! Every function here takes a built [`crate::model::Page`] and writes it out.
//! There is no lookup, no sorting and no counting in this layer: if a template
//! wants to know something, the model already knows it.
//!
//! The ids and classes are a contract with `static/review.js`. Changing one
//! here changes the script too.

pub mod bar;
pub mod board;
pub mod diff;
pub mod document;
pub mod rail;

pub use bar::bar;
pub use board::{board, board_page};
pub use diff::{diff, file_table, thread_row};
pub use document::{missing, page};
pub use rail::rail;

/// The minus a diff count is written with, so a removed count is not read as
/// a hyphen.
pub(crate) const MINUS: &str = "\u{2212}";
