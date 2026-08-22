//! The board: everything proposed, in three groups.
//!
//! It is one fragment with an id, so the stream replaces it whole and the page
//! around it is never rebuilt.

use maud::{Markup, html};

use crate::model::Board;
use crate::model::board::Entry;
use crate::render::MINUS;
use crate::render::document::shell;

/// The board as a whole page.
#[must_use]
pub fn board_page(rows: &Board) -> Markup {
    let body = html! {
        body data-board data-fp=(rows.fingerprint()) {
            header id="bar" class="board" {
                div class="who" {
                    a class="mark" href="/" { "githerb" }
                    h1 { "proposals" }
                }
                div class="actions" {
                    button data-density title="comfortable or compact" { "density" }
                button data-theme title="light or dark" { "theme" }
                }
            }
            (board(rows))
        }
    };
    shell("proposals", &body)
}

/// The board fragment, which is what the stream pushes.
#[must_use]
pub fn board(rows: &Board) -> Markup {
    html! {
        main id="board" {
            (group("in review", rows.open(), "Nothing proposed yet."))
            (group("got in", rows.landed(), "Nothing has landed."))
            (group("did not", rows.abandoned(), ""))
        }
    }
}

/// One group, or the sentence that stands in for it when it is empty.
fn group(heading: &str, entries: &[Entry], empty: &str) -> Markup {
    html! {
        @if !entries.is_empty() || !empty.is_empty() {
            section class="pane" {
                h2 { (heading) @if !entries.is_empty() { span class="n" { (entries.len()) } } }
                @if entries.is_empty() {
                    p class="none" { (empty) }
                } @else {
                    ol class="proposals" {
                        @for entry in entries { (row(entry)) }
                    }
                }
            }
        }
    }
}

/// One proposal.
fn row(entry: &Entry) -> Markup {
    html! {
        li {
            a href={ "/p/" (entry.id()) } { (entry.title()) }
            span class="meta" {
                span class="onto" { "onto " (entry.target()) }
                span class="rev" { "r" (entry.revision()) }
                @if entry.added() > 0 { span class="added" { "+" (entry.added()) } }
                @if entry.removed() > 0 { span class="removed" { (MINUS) (entry.removed()) } }
                @if entry.notes() > 0 {
                    span class="notes" {
                        (entry.notes()) " note" @if entry.notes() != 1 { "s" }
                    }
                }
                span class="checks" { (entry.checks()) }
                time { (entry.at()) }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{at, sha};
    use review::{Branch, Proposal, ProposalId};
    use std::collections::HashMap;

    /// A proposal of that name, in that state.
    fn named(id: &str, minutes: i64) -> Proposal {
        Proposal::open(
            ProposalId::parse(id).unwrap(),
            "A slice",
            Branch::parse("main").unwrap(),
            sha('a'),
            sha('b'),
            at(minutes),
        )
        .unwrap()
    }

    #[test]
    fn the_board_names_the_three_groups() {
        let mut landed = named("older", 10);
        landed.mark_landed(at(30));
        let board = Board::build(&[named("newer", 50), landed], &HashMap::new());
        let html = self::board(&board).into_string();
        assert!(html.starts_with("<main id=\"board\">"), "{html}");
        assert!(html.contains("in review"), "{html}");
        assert!(html.contains("got in"), "{html}");
        assert!(html.contains(r#"<a href="/p/newer">A slice</a>"#), "{html}");
        assert!(html.contains(r#"<a href="/p/older">A slice</a>"#), "{html}");
    }

    #[test]
    fn an_empty_board_says_what_is_missing() {
        let html = self::board(&Board::default()).into_string();
        assert!(html.contains("Nothing proposed yet."), "{html}");
        assert!(html.contains("Nothing has landed."), "{html}");
        assert!(!html.contains("did not"), "an empty group nobody needs");
    }

    #[test]
    fn the_board_page_is_a_document() {
        let html = board_page(&Board::default()).into_string();
        assert!(html.starts_with("<!DOCTYPE html>"), "{html}");
        assert!(html.contains("<body data-board data-fp="), "{html}");
        assert!(
            html.contains("<title>proposals · githerb</title>"),
            "{html}"
        );
    }
}
