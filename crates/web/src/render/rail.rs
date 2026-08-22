//! The rail: the files, what the change decides, every note, and the work log.
//!
//! Like the bar it is swapped whole over the event stream, which is why a note
//! left anywhere costs the rail and the bar and nothing else.

use maud::{Markup, html};

use crate::model::{Page, Thread};
use crate::render::MINUS;

/// The whole rail.
#[must_use]
pub fn rail(page: &Page) -> Markup {
    html! {
        aside id="rail" {
            section class="pane" {
                h2 { "files" span class="n" { (page.files().len()) } }
                nav class="files" {
                    @for file in page.files() {
                        a href={ "#f-" (file.index()) } {
                            span class="p" { (file.path()) }
                            @if file.added() > 0 { span class="added" { "+" (file.added()) } }
                            @if file.removed() > 0 {
                                span class="removed" { (MINUS) (file.removed()) }
                            }
                            @if file.noted() { span class="dot" {} }
                        }
                    }
                }
            }
            @if !page.decisions().is_empty() {
                section class="pane" {
                    h2 { "decides" span class="n" { (page.decisions().len()) } }
                    ol class="decisions" {
                        @for decision in page.decisions() {
                            li id={ "d" (decision.number()) } {
                                @match decision.row() {
                                    Some(row) => a href={ "#" (row) } { (decision.title()) },
                                    None => span class="t" { (decision.title()) },
                                }
                                @if let Some(at) = decision.at() {
                                    span class="at" { (at) }
                                }
                                dl {
                                    @if let Some(surface) = decision.surface() {
                                        dt { "where" } dd { (surface) }
                                    }
                                    dt { "was" } dd { (decision.before()) }
                                    dt { "now" } dd { (decision.after()) }
                                    dt { "call" } dd { (decision.call()) }
                                    @if let Some(rejected) = decision.rejected() {
                                        dt { "instead of" } dd { (rejected) }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            (notes(page))
            @if !page.timeline().is_empty() {
                section class="pane" {
                    h2 { "agent" }
                    ol class="timeline" {
                        @for entry in page.timeline() {
                            li class=(entry.phase()) {
                                time { (entry.at()) }
                                span class="who" { (entry.agent()) }
                                (entry.task()) " " (entry.phase())
                                @if let Some(note) = entry.note() {
                                    span class="note" { (note) }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Every note in one place: the open ones, then what fell off the head, then
/// what has been answered.
fn notes(page: &Page) -> Markup {
    let open = page.notes_open();
    let earlier = page.notes_earlier();
    let resolved = page.notes_resolved();
    html! {
        section class="pane" {
            h2 { "notes" span class="n" { (page.open_notes()) } }
            @if open.is_empty() && earlier.is_empty() && resolved.is_empty() {
                p class="none" { "Nothing noted yet. Select lines in the gutter to leave one." }
            }
            @if !open.is_empty() {
                ul class="threads" { @for thread in &open { (item(thread)) } }
            }
            @if !earlier.is_empty() {
                details {
                    summary { "earlier revisions (" (earlier.len()) ")" }
                    ul class="threads" { @for thread in &earlier { (item(thread)) } }
                }
            }
            @if !resolved.is_empty() {
                details {
                    summary { "resolved (" (resolved.len()) ")" }
                    ul class="threads done" { @for thread in &resolved { (item(thread)) } }
                }
            }
        }
    }
}

/// One thread of the note list: where it points, and the whole conversation
/// with its buttons, so an answer can be read and given from here even when
/// the line it was left on is no longer in the diff.
fn item(thread: &Thread) -> Markup {
    html! {
        li class=(if thread.stale().is_some() { "thread stale" } else { "thread" })
            id={ "s-" (thread.id()) } {
            a class="at" href={ "#t-" (thread.id()) } { (thread.file()) ":" (thread.line()) }
            (crate::render::diff::conversation(thread))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{anchor, note, patch, proposal, sha};
    use crate::model::Page;
    use review::{Chunk, Record, Side};

    /// A note that fell off the head has no row in the diff, so the rail is the
    /// only place its conversation can be read and answered from.
    #[test]
    fn the_rail_carries_the_whole_thread_answers_and_buttons_included() {
        let mut proposal = proposal();
        let first = proposal.head().sha().clone();
        let id = note(
            &mut proposal,
            &first,
            "README.md",
            Side::New,
            1,
            1,
            "drop it",
        );
        proposal
            .apply(Record::Reply(
                review::Reply::new(
                    id.clone(),
                    first.clone(),
                    "folded it into the first line",
                    review::Author::parse("githerb-run").unwrap(),
                    review::Timestamp::from_unix(1_767_225_600),
                )
                .unwrap(),
            ))
            .unwrap();
        proposal.add_revision(sha('c')).unwrap();
        let html = rail(&Page::build(&proposal, &patch(), &[], None)).into_string();
        assert!(html.contains(&format!("id=\"s-{id}\"")), "{html}");
        assert!(html.contains("folded it into the first line"), "{html}");
        assert!(html.contains("githerb-run"), "{html}");
        assert!(html.contains(&format!("data-reply=\"{id}\"")), "{html}");
        assert!(html.contains(&format!("data-resolve=\"{id}\"")), "{html}");
    }

    #[test]
    fn a_note_that_fell_off_the_head_is_folded_away_from_the_open_ones() {
        let mut proposal = proposal();
        let first = proposal.head().sha().clone();
        note(
            &mut proposal,
            &first,
            "README.md",
            Side::New,
            1,
            1,
            "drop it",
        );
        proposal.add_revision(sha('c')).unwrap();
        let html = rail(&Page::build(&proposal, &patch(), &[], None)).into_string();
        assert!(
            html.contains("<summary>earlier revisions (1)</summary>"),
            "{html}"
        );
        assert!(html.contains(">README.md:1</a>"), "{html}");
        assert!(
            html.contains("<h2>notes<span class=\"n\">0</span></h2>"),
            "{html}"
        );
    }

    #[test]
    fn a_decision_lists_how_it_was_and_how_it_is() {
        let mut proposal = proposal();
        let chunk = Chunk::new(
            "The title says what it is",
            Some("docs"),
            "it said demo",
            "it says what it is",
            "rename it in place",
            Some("a second title line"),
        )
        .unwrap()
        .anchored(anchor("README.md", Side::New, 1, 1));
        proposal.apply(Record::Chunk(chunk)).unwrap();
        let html = rail(&Page::build(&proposal, &patch(), &[], None)).into_string();
        assert!(
            html.contains(r##"<li id="d1"><a href="#L-0-1">The title says what it is</a>"##),
            "{html}"
        );
        assert!(html.contains("<dt>where</dt><dd>docs</dd>"), "{html}");
        assert!(
            html.contains("<dt>instead of</dt><dd>a second title line</dd>"),
            "{html}"
        );
        assert!(
            html.contains(r#"<span class="at">README.md:1</span>"#),
            "{html}"
        );
    }

    #[test]
    fn the_files_are_listed_with_their_counts() {
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
        let html = rail(&Page::build(&proposal, &patch(), &[], None)).into_string();
        assert!(
            html.contains(r##"<a href="#f-0"><span class="p">README.md</span>"##),
            "{html}"
        );
        assert!(
            html.contains(r#"<span class="dot"></span>"#),
            "the noted file is marked"
        );
        assert!(
            html.contains("<h2>files<span class=\"n\">2</span></h2>"),
            "{html}"
        );
    }

    #[test]
    fn a_page_with_nothing_on_it_says_how_to_start() {
        let html = rail(&Page::build(&proposal(), &patch(), &[], None)).into_string();
        assert!(html.contains("Select lines in the gutter"), "{html}");
        assert!(!html.contains("<ol class=\"timeline\">"), "no work yet");
    }
}
