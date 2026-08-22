//! The bar: what this proposal is, what the checks say, who is on it and the
//! three buttons that move it.
//!
//! It is swapped whole over the event stream, so everything it needs is inside
//! the element that carries its id.

use maud::{Markup, html};
use review::State;

use crate::model::{Origin, Page};
use crate::render::MINUS;

/// The whole bar.
#[must_use]
pub fn bar(page: &Page) -> Markup {
    html! {
        header id="bar" {
            div class="who" {
                a class="mark" href="/" { "githerb" }
                h1 { (page.title()) }
            }
            div class="facts" {
                span class="rev" { "r" (page.head()) }
                span class="onto" { "onto " (page.target()) }
                span class="added" { "+" (page.added()) }
                span class="removed" { (MINUS) (page.removed()) }
                @if page.revised() {
                    (origins(page))
                }
            }
            div class="state" {
                @if let Some(why) = blocking(page) {
                    span class="blocked" { (why) }
                }
                @if !page.checks().is_empty() {
                    ul class="checks" {
                        @for check in page.checks() {
                            li class=(check.state().class()) {
                                (check.name()) " · " (verdict(check))
                            }
                        }
                    }
                }
                span class={ "agent " (page.agent().state().class()) }
                    title=[page.agent().note()] { (page.agent().text()) }
            }
            div class="actions" {
                @match page.state() {
                    State::Open => (actions(page)),
                    State::Landed => span class="closed" { "landed onto " (page.target()) },
                    State::Abandoned => span class="closed" { "abandoned" },
                }
                a href={ "/p/" (page.id()) "/handover" } data-handover { "copy handover" }
                button data-theme title="light or dark" { "theme" }
            }
        }
    }
}

/// Why an open proposal cannot land. A closed one says so in the actions
/// instead, and repeating it in the bar would only crowd the title out.
fn blocking(page: &Page) -> Option<&str> {
    match page.state() {
        State::Open => page.blocked(),
        State::Landed | State::Abandoned => None,
    }
}

/// The buttons that only mean something while the proposal is open.
fn actions(page: &Page) -> Markup {
    let blocked = page.blocked();
    html! {
        @if page.open_notes() > 0 {
            button data-dispatch {
                "Send " (page.open_notes()) " note" (plural(page.open_notes())) " to the agent"
            }
        }
        button data-land class="primary" disabled[blocked.is_some()] title=[blocked] {
            "Land onto " (page.target())
        }
        button data-abandon { "Abandon" }
    }
}

/// Where the diff is measured from, and the head it always ends at.
fn origins(page: &Page) -> Markup {
    html! {
        nav class="origins" {
            @for origin in page.origins() {
                a href=(link(page.id(), origin)) class=[origin.active().then_some("active")] {
                    (origin.label())
                }
            }
            span class="head" { "r" (page.head()) }
        }
    }
}

/// The page url that measures the diff from that origin.
fn link(id: &str, origin: &Origin) -> String {
    match origin.since() {
        0 => format!("/p/{id}"),
        since => format!("/p/{id}?since={since}"),
    }
}

/// What one check chip says after its name.
fn verdict(check: &crate::model::CheckRow) -> String {
    match check.state() {
        crate::model::CheckState::Passed => format!("{}s", check.seconds()),
        crate::model::CheckState::Failed if check.seconds() > 0 => {
            format!("failed {}s", check.seconds())
        }
        crate::model::CheckState::Failed => "failed".to_owned(),
        crate::model::CheckState::Missing => "missing".to_owned(),
    }
}

/// The `s` on a count of notes.
fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{at, note, patch, proposal, sha};
    use crate::model::Page;
    use review::{CheckName, Side};

    #[test]
    fn the_land_button_says_why_it_is_disabled() {
        let mut proposal = proposal();
        let head = proposal.head().sha().clone();
        note(&mut proposal, &head, "README.md", Side::New, 1, 1, "no");
        let html = bar(&Page::build(&proposal, &patch(), &[], None)).into_string();
        assert!(
            html.contains("<span class=\"blocked\">the head revision still has open comments: 1"),
            "{html}"
        );
        assert!(
            html.contains("<button data-land class=\"primary\" disabled"),
            "{html}"
        );
        assert!(
            html.contains("<button data-dispatch>Send 1 note to the agent</button>"),
            "{html}"
        );
    }

    #[test]
    fn a_clear_proposal_can_be_landed() {
        let html = bar(&Page::build(&proposal(), &patch(), &[], None)).into_string();
        assert!(
            html.contains("<button data-land class=\"primary\">Land onto main</button>"),
            "{html}"
        );
        assert!(!html.contains("data-dispatch"), "nothing to hand over");
    }

    #[test]
    fn a_required_check_that_never_ran_is_a_chip_of_its_own() {
        let required = vec![CheckName::parse("gate").unwrap()];
        let html = bar(&Page::build(&proposal(), &patch(), &required, None)).into_string();
        assert!(
            html.contains("<li class=\"missing\">gate · missing</li>"),
            "{html}"
        );
    }

    #[test]
    fn a_landed_proposal_offers_nothing_to_press() {
        let mut proposal = proposal();
        proposal.mark_landed(at(90));
        let html = bar(&Page::build(&proposal, &patch(), &[], None)).into_string();
        assert!(html.contains("landed onto main"), "{html}");
        assert!(!html.contains("data-land"), "{html}");
        assert!(!html.contains("data-abandon"), "{html}");
    }

    #[test]
    fn the_origins_strip_arrives_with_the_second_revision() {
        let one = bar(&Page::build(&proposal(), &patch(), &[], None)).into_string();
        assert!(!one.contains("origins"), "one revision has no other end");

        let mut proposal = proposal();
        proposal.add_revision(sha('c')).unwrap();
        let two = bar(&Page::build(&proposal, &patch(), &[], Some(1))).into_string();
        assert!(
            two.contains(r#"<a href="/p/demo?since=1" class="active">r1</a>"#),
            "{two}"
        );
        assert!(two.contains(r#"<a href="/p/demo">base</a>"#), "{two}");
        assert!(two.contains(r#"<span class="head">r2</span>"#), "{two}");
    }

    #[test]
    fn the_agent_chip_says_what_the_log_says() {
        let html = bar(&Page::build(&proposal(), &patch(), &[], None)).into_string();
        assert!(
            html.contains(r#"<span class="agent idle">no agent on it</span>"#),
            "{html}"
        );
    }
}
