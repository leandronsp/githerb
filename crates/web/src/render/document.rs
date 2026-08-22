//! The two documents: the review page and the one that says there is nothing
//! at that address.
//!
//! The head is the same for both. It carries the theme bootstrap inline,
//! because a theme applied after paint is a white flash, and links the two
//! static assets with a content hash so they are cached for a year and still
//! change the moment they do.

use maud::{DOCTYPE, Markup, PreEscaped, html};

use crate::assets::asset_hash;
use crate::model::Page;
use crate::render::{bar, diff, rail};

/// Applied before the first paint, so the page never flashes the wrong theme.
const THEME: &str = r#"try{var t=localStorage.getItem("githerb:theme");if(t)document.documentElement.dataset.theme=t}catch(e){}"#;

/// The whole review page.
#[must_use]
pub fn page(page: &Page) -> Markup {
    let body = html! {
        body data-proposal=(page.id()) data-head=(page.head()) data-fp=(page.fingerprint()) {
            (bar::bar(page))
            div id="frame" {
                (rail::rail(page))
                (diff::diff(page))
            }
            template id="composer" {
                form {
                    textarea name="body" rows="3" placeholder="what needs to change here?" {}
                    footer {
                        span class="where" {}
                        button type="submit" { "Leave note" }
                        button type="button" data-cancel { "Cancel" }
                    }
                }
            }
            output id="toast" hidden {}
        }
    };
    shell(page.title(), &body)
}

/// Nothing lives at that address.
#[must_use]
pub fn missing(what: &str) -> Markup {
    let body = html! {
        body class="plain" {
            main class="empty" {
                h1 { "not found" }
                p { (what) }
                p { a href="/" { "all proposals" } }
            }
        }
    };
    shell("not found", &body)
}

/// The document every page is written into.
pub(crate) fn shell(title: &str, body: &Markup) -> Markup {
    let hash = asset_hash();
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) " · githerb" }
                script { (PreEscaped(THEME)) }
                link rel="stylesheet" href={ "/static/review.css?v=" (hash) };
                script src={ "/static/review.js?v=" (hash) } defer {}
            }
            (body)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{note, patch, proposal};
    use crate::model::Page;
    use review::Side;

    /// The whole review page of the sample proposal, with one note on it.
    fn rendered() -> String {
        let mut proposal = proposal();
        let head = proposal.head().sha().clone();
        note(
            &mut proposal,
            &head,
            "README.md",
            Side::New,
            1,
            1,
            "drop the suffix",
        );
        let page = Page::build(&proposal, &patch(), &[], None);
        self::page(&page).into_string()
    }

    #[test]
    fn the_body_carries_the_proposal_and_the_fingerprint() {
        let page = Page::build(&proposal(), &patch(), &[], None);
        let html = self::page(&page).into_string();
        let expected = format!(
            "<body data-proposal=\"demo\" data-head=\"1\" data-fp=\"{}\">",
            page.fingerprint()
        );
        assert!(html.contains(&expected), "{html}");
    }

    #[test]
    fn the_page_is_a_bar_a_rail_and_a_diff() {
        let html = rendered();
        for part in [
            "<header id=\"bar\">",
            "<aside id=\"rail\">",
            "<main id=\"diff\">",
            "<template id=\"composer\">",
            "<output id=\"toast\" hidden></output>",
        ] {
            assert!(html.contains(part), "no {part} in {html}");
        }
    }

    #[test]
    fn every_file_is_a_section_and_every_line_a_row() {
        let html = rendered();
        assert!(html.contains(r#"<section class="file" id="f-0" data-path="README.md">"#));
        assert!(
            html.contains(r#"<section class="file" id="f-1" data-path="cmd/githerb/extra.go">"#)
        );
        assert_eq!(html.matches("<tr id=\"L-0-").count(), 5);
        assert_eq!(html.matches("<tr id=\"L-1-").count(), 3);
        assert_eq!(html.matches("<tr class=\"thread-row\"").count(), 1);
    }

    #[test]
    fn the_assets_are_linked_with_the_version_they_were_built_with() {
        let html = rendered();
        let hash = asset_hash();
        assert!(
            html.contains(&format!("/static/review.css?v={hash}")),
            "{html}"
        );
        assert!(
            html.contains(&format!("/static/review.js?v={hash}")),
            "{html}"
        );
        assert!(
            html.contains("localStorage.getItem(\"githerb:theme\")"),
            "{html}"
        );
    }

    #[test]
    fn nothing_at_that_address_is_still_a_page() {
        let html = missing("no proposal is called that").into_string();
        assert!(html.contains("<h1>not found</h1>"), "{html}");
        assert!(html.contains("no proposal is called that"), "{html}");
    }
}
