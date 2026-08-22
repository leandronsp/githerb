//! The stylesheet and the script, compiled into the binary.
//!
//! There is no CDN and no build step: two files, served from memory, named
//! with a hash of their own contents so the browser can cache them for a year
//! and still pick up a change the moment there is one. The old surface inlined
//! sixteen kilobytes of css into every response instead.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::OnceLock;

/// The stylesheet.
const CSS: &str = include_str!("static/review.css");

/// The script.
const JS: &str = include_str!("static/review.js");

/// The version both asset urls carry, derived from what the assets say.
///
/// Stable for a given binary, which is what makes an immutable cache header
/// honest.
#[must_use]
pub fn asset_hash() -> &'static str {
    static HASH: OnceLock<String> = OnceLock::new();
    HASH.get_or_init(|| {
        let mut hasher = DefaultHasher::new();
        CSS.hash(&mut hasher);
        JS.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    })
}

/// The content type and body of a static asset, and nothing for a path that
/// is not one.
#[must_use]
pub fn static_asset(path: &str) -> Option<(&'static str, &'static str)> {
    match path {
        "/static/review.css" => Some(("text/css; charset=utf-8", CSS)),
        "/static/review.js" => Some(("text/javascript; charset=utf-8", JS)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_asset_is_served_with_its_type() {
        assert_eq!(
            static_asset("/static/review.css").map(|(mime, _)| mime),
            Some("text/css; charset=utf-8")
        );
        assert_eq!(
            static_asset("/static/review.js").map(|(mime, _)| mime),
            Some("text/javascript; charset=utf-8")
        );
        assert_eq!(static_asset("/static/nothing.css"), None);
    }

    #[test]
    fn the_hash_is_the_same_every_time_it_is_asked_for() {
        assert_eq!(asset_hash(), asset_hash());
        assert_eq!(asset_hash().len(), 16);
    }
}
