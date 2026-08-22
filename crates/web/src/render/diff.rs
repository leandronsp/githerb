//! The diff itself, and the two fragments that are pushed or fetched into it.
//!
//! This is the only markup written per line of code, so it is the only markup
//! whose size matters. A row is one `<tr>` with three cells, no data
//! attributes and no class at all when it is a context line: the script reads
//! the file off the section, the side off the cell's class and the number off
//! the cell's text. The old page spent 560 bytes a row on attributes that
//! repeated what the dom already said.

use maud::{Markup, html};

use crate::model::{FileView, Page, Row, RowKind, Thread};
use crate::render::MINUS;

/// Every file, in the order git listed them.
#[must_use]
pub fn diff(page: &Page) -> Markup {
    html! {
        main id="diff" {
            @for file in page.files() {
                (section(page, file))
            }
            @if page.files().is_empty() {
                p class="none" { "This revision changes nothing." }
            }
        }
    }
}

/// One file's table, which is what a collapsed file fetches.
#[must_use]
pub fn file_table(page: &Page, index: usize) -> Option<Markup> {
    page.file(index).map(|file| table(page, file))
}

/// One thread, standalone, which is what the event stream pushes.
#[must_use]
pub fn thread_row(thread: &Thread) -> Markup {
    html! {
        tr class="thread-row" id={ "t-" (thread.id()) } data-after=(thread.row()) {
            td colspan="3" {
                div class=(if thread.stale().is_some() { "thread stale" } else { "thread" }) {
                    div class="turn" {
                        span class="who" { (thread.author()) }
                        time { (thread.at()) }
                        @if let Some(on) = thread.stale() { span class="on" { (on) } }
                        p { (thread.body()) }
                    }
                    @for answer in thread.answers() {
                        div class="turn answer" {
                            span class="who" { (answer.author()) }
                            p { (answer.body()) }
                        }
                    }
                    p class="doing" {
                        button data-reply=(thread.id()) { "reply" }
                        @if !thread.resolved() {
                            button data-resolve=(thread.id()) { "resolve" }
                        }
                    }
                }
            }
        }
    }
}

/// One file: its header, and either its rows or an offer to fetch them.
fn section(page: &Page, file: &FileView) -> Markup {
    html! {
        section class=(if file.collapsed() { "file folded" } else { "file" })
            id={ "f-" (file.index()) } data-path=(file.path()) {
            header class="file-head" {
                span class="path" { (file.path()) }
                @if file.added() > 0 { span class="added" { "+" (file.added()) } }
                @if file.removed() > 0 { span class="removed" { (MINUS) (file.removed()) } }
                button class="fold" aria-expanded=(if file.collapsed() { "false" } else { "true" })
                    title="fold" { "\u{2303}" }
            }
            @if file.binary() {
                p class="binary" { "binary file" }
            } @else if file.collapsed() {
                button class="load" data-file=(file.index()) {
                    "show " (file.lines()) " lines"
                }
            } @else {
                (table(page, file))
            }
        }
    }
}

/// The rows of one file.
fn table(page: &Page, file: &FileView) -> Markup {
    html! {
        table class="hunks" {
            colgroup { col class="g"; col class="g"; col; }
            @for hunk in file.hunks() {
                tr class="hunk" { td colspan="3" { (hunk.header()) } }
                @for row in hunk.rows() {
                    (line(row))
                    @for position in row.threads() {
                        @if let Some(thread) = page.thread(*position) {
                            (thread_row(thread))
                        }
                    }
                }
            }
        }
    }
}

/// One line of code. Everything here is paid for on every line of every file.
fn line(row: &Row) -> Markup {
    html! {
        tr id=(row.id()) class=[class_of(row)] {
            td class="o" { @if let Some(number) = row.old_number() { (number) } }
            td class="n" { @if let Some(number) = row.new_number() { (number) } }
            td class="c" {
                (row.text())
                @for number in row.decisions() { span class="chip" { "d" (number) } }
                @for why in row.why() { span class="chip why" title=(why) { "why" } }
            }
        }
    }
}

/// The row's classes, or nothing at all for a plain context line.
fn class_of(row: &Row) -> Option<&'static str> {
    let decided = !row.decisions().is_empty();
    Some(match (row.kind(), row.noted(), decided) {
        (RowKind::Context, false, false) => return None,
        (RowKind::Context, true, false) => "noted",
        (RowKind::Context, false, true) => "decided",
        (RowKind::Context, true, true) => "noted decided",
        (RowKind::Added, false, false) => "add",
        (RowKind::Added, true, false) => "add noted",
        (RowKind::Added, false, true) => "add decided",
        (RowKind::Added, true, true) => "add noted decided",
        (RowKind::Removed, false, false) => "del",
        (RowKind::Removed, true, false) => "del noted",
        (RowKind::Removed, false, true) => "del decided",
        (RowKind::Removed, true, true) => "del noted decided",
    })
}

#[cfg(test)]
mod tests {
    use std::fmt::Write;

    use super::*;
    use crate::fixtures::{note, patch, proposal};
    use crate::model::Page;
    use review::Side;

    /// Every `<tr>` of a rendered table that carries a line of code.
    fn code_rows(html: &str) -> Vec<&str> {
        html.match_indices("<tr id=\"L-")
            .map(|(start, _)| {
                let rest = &html[start..];
                let end = rest.find("</tr>").unwrap() + "</tr>".len();
                &rest[..end]
            })
            .collect()
    }

    /// A diff of one file whose lines are the length real code has.
    fn sample() -> patch::Patch {
        let lines = [
            "    let mut rows = Vec::new();",
            "    for line in hunk.lines() {",
            "        rows.push(row(line));",
            "    }",
            "    Ok(rows)",
        ];
        let mut diff = String::from("diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n");
        diff.push_str("+++ b/src/lib.rs\n@@ -1,5 +1,5 @@\n");
        for line in lines {
            let _ = writeln!(diff, " {line}");
        }
        patch::parse(&diff).unwrap()
    }

    #[test]
    fn a_row_of_a_typical_diff_stays_under_the_ceiling() {
        let page = Page::build(&proposal(), &sample(), &[], None);
        let html = diff(&page).into_string();
        let rows = code_rows(&html);
        let widest = rows.iter().map(|row| row.len()).max().unwrap();
        assert_eq!(rows.len(), 5);
        assert!(widest <= 110, "a row costs {widest} bytes: {rows:?}");
    }

    #[test]
    fn a_context_row_carries_no_class_and_the_gutters_carry_only_numbers() {
        let page = Page::build(&proposal(), &patch(), &[], None);
        let html = diff(&page).into_string();
        assert!(
            html.contains(
                r#"<tr id="L-0-2"><td class="o">2</td><td class="n">2</td><td class="c">"#
            ),
            "{html}"
        );
        assert!(
            html.contains(r#"<tr id="L-0-0" class="del"><td class="o">1</td><td class="n"></td>"#),
            "{html}"
        );
    }

    #[test]
    fn a_thread_row_follows_the_row_its_note_ends_on() {
        let mut proposal = proposal();
        let head = proposal.head().sha().clone();
        note(
            &mut proposal,
            &head,
            "README.md",
            Side::New,
            2,
            3,
            "name these",
        );
        let page = Page::build(&proposal, &patch(), &[], None);
        let html = diff(&page).into_string();
        let id = page.threads()[0].id().to_string();
        let expected = format!("</tr><tr class=\"thread-row\" id=\"t-{id}\" data-after=\"L-0-3\">");
        assert!(html.contains(&expected), "{html}");
        assert!(html.contains("<button data-resolve=\""), "{html}");
    }

    #[test]
    fn a_thread_row_renders_the_same_standing_alone() {
        let mut proposal = proposal();
        let head = proposal.head().sha().clone();
        note(
            &mut proposal,
            &head,
            "README.md",
            Side::New,
            1,
            1,
            "drop it",
        );
        let page = Page::build(&proposal, &patch(), &[], None);
        let alone = thread_row(page.threads().first().unwrap()).into_string();
        assert!(diff(&page).into_string().contains(&alone), "{alone}");
    }

    #[test]
    fn a_resolved_note_has_no_resolve_button() {
        let mut proposal = proposal();
        let head = proposal.head().sha().clone();
        let id = note(
            &mut proposal,
            &head,
            "README.md",
            Side::New,
            1,
            1,
            "drop it",
        );
        proposal
            .apply(review::Record::Resolve(review::Resolution::new(
                id,
                crate::fixtures::author("leandro"),
                crate::fixtures::at(50),
            )))
            .unwrap();
        let page = Page::build(&proposal, &patch(), &[], None);
        let html = thread_row(page.threads().first().unwrap()).into_string();
        assert!(!html.contains("data-resolve"), "{html}");
        assert!(html.contains("data-reply"), "{html}");
    }

    #[test]
    fn a_collapsed_file_offers_to_fetch_its_lines() {
        let mut diff_text = String::from("diff --git a/big.txt b/big.txt\n--- a/big.txt\n");
        diff_text.push_str("+++ b/big.txt\n@@ -0,0 +1,3100 @@\n");
        for line in 0..3100 {
            let _ = writeln!(diff_text, "+line {line}");
        }
        let patch = patch::parse(&diff_text).unwrap();
        let page = Page::build(&proposal(), &patch, &[], None);
        let html = diff(&page).into_string();
        assert!(html.contains(r#"<button class="load" data-file="0">show 3100 lines</button>"#));
        assert!(!html.contains("<table"), "the table is left for later");
        assert!(
            file_table(&page, 0)
                .unwrap()
                .into_string()
                .contains("<table class=\"hunks\">")
        );
        assert!(file_table(&page, 9).is_none(), "no such file");
    }
}
