//! What the terminal reads: a listing, a proposal, a thread.
//!
//! The columns are fixed on purpose. A listing that lines up is one a person
//! can scan for the one row that is different, and both surfaces say the same
//! sentence about who is on a proposal, so the browser and the terminal never
//! disagree.

use std::fmt::Write as _;

use review::{Check, Comment, Proposal, Record};

/// Whether a thread shows only what is still in the way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// The notes on the head revision that nobody resolved: what blocks.
    Open,
    /// Every note nobody resolved, from any revision: what to read.
    All,
}

/// Whether a thread is for a person or for a program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// One readable line per note.
    Text,
    /// The wire bytes, one record per line, for an agent to read.
    Json,
}

/// Every proposal, one row each.
#[must_use]
pub fn list(proposals: &[Proposal]) -> String {
    if proposals.is_empty() {
        return "no proposals yet\n".to_owned();
    }

    let mut text = String::new();

    for proposal in proposals {
        let _ignored = writeln!(
            text,
            "{:<44} {:<9} r{}  {:2} open  {:<8} onto {}",
            proposal.id().as_str(),
            proposal.state().as_str(),
            proposal.head().number(),
            proposal.open_comments().len(),
            proposal.check_summary().to_string(),
            proposal.target().as_str()
        );
    }

    text
}

/// One proposal: what it is, where it goes, what ran on it and what is still
/// in the way.
#[must_use]
pub fn show(proposal: &Proposal) -> String {
    let revisions = proposal.revisions();
    let mut text = format!(
        "{}\n{}\n\nonto {}, cut from {}\nstate {}, revision {} of {}\n\n",
        proposal.id(),
        proposal.title(),
        proposal.target(),
        proposal.base().short(),
        proposal.state().as_str(),
        proposal.head().number(),
        revisions.len()
    );

    for revision in revisions {
        let _ignored = writeln!(
            text,
            "  r{:<3} {}",
            revision.number(),
            revision.sha().short()
        );
    }

    text.push('\n');

    let checks = proposal.checks();
    for check in &checks {
        let _ignored = writeln!(
            text,
            "{:<16} {} in {}s",
            check.name().as_str(),
            check.status().as_str(),
            check.seconds()
        );
    }
    if !checks.is_empty() {
        text.push('\n');
    }

    let _ignored = write!(text, "{}\n\n", proposal.agent_line());

    let open = proposal.open_comments();
    if open.is_empty() {
        text.push_str("nothing open\n");

        return text;
    }

    for comment in open {
        text.push_str(&thread_line(comment));
    }

    text
}

/// The notes on a proposal, for a person or for an agent.
#[must_use]
pub fn comments(proposal: &Proposal, scope: Scope, shape: Shape) -> String {
    let notes = match scope {
        Scope::Open => proposal.open_comments(),
        Scope::All => proposal.conversation(),
    };

    notes
        .into_iter()
        .map(|comment| match shape {
            Shape::Text => format!(
                "{}  {}:{}  {}\n",
                comment.id(),
                comment.anchor().file(),
                comment.anchor().span().start(),
                comment.body()
            ),
            Shape::Json => format!("{}\n", Record::Comment(comment.clone()).to_line()),
        })
        .collect()
}

/// One result, as `githerb check` reports it while it goes.
#[must_use]
pub fn check_line(check: &Check) -> String {
    format!(
        "{:<16} {:<7} {}s",
        check.name().as_str(),
        check.status().as_str(),
        check.seconds()
    )
}

/// A note the way `githerb show` prints it: where it is, then what it says.
fn thread_line(comment: &Comment) -> String {
    let span = comment.anchor().span();
    let range = if span.lines() > 1 {
        format!("{}:{}", span.start(), span.end())
    } else {
        span.start().to_string()
    };

    format!(
        "{}  {}:{range}\n  {}\n",
        comment.id(),
        comment.anchor().file(),
        comment.body()
    )
}

#[cfg(test)]
mod tests {
    use review::{
        Anchor, Author, Branch, CheckName, CheckStatus, Comment, FilePath, ProposalId, Side, Span,
        Timestamp,
    };

    use super::*;

    fn sha(letter: char) -> review::Sha {
        review::Sha::parse(&std::iter::repeat_n(letter, 40).collect::<String>()).unwrap()
    }

    fn proposal() -> Proposal {
        Proposal::open(
            ProposalId::parse("land-the-gate-aaaaaaa").unwrap(),
            "Land the gate",
            Branch::parse("main").unwrap(),
            sha('b'),
            sha('a'),
            Timestamp::from_unix(1_760_000_000),
        )
        .unwrap()
    }

    fn note(proposal: &Proposal) -> Comment {
        Comment::new(
            proposal.head().sha().clone(),
            Anchor::new(
                FilePath::parse("src/main.rs").unwrap(),
                Span::new(Side::New, 12, 14).unwrap(),
            ),
            "say why",
            Author::parse("ada").unwrap(),
            Timestamp::from_unix(1_760_000_100),
        )
        .unwrap()
    }

    #[test]
    fn nothing_proposed_says_so() {
        assert_eq!(list(&[]), "no proposals yet\n");
    }

    #[test]
    fn a_row_lines_up_with_the_next_one() {
        let row = list(&[proposal()]);

        assert_eq!(
            row,
            "land-the-gate-aaaaaaa                        open      r1   0 open  no checks onto main\n"
        );
    }

    #[test]
    fn a_proposal_with_nothing_open_says_so() {
        assert_eq!(
            show(&proposal()),
            concat!(
                "land-the-gate-aaaaaaa\n",
                "Land the gate\n",
                "\n",
                "onto main, cut from bbbbbbb\n",
                "state open, revision 1 of 1\n",
                "\n",
                "  r1   aaaaaaa\n",
                "\n",
                "no agent on it\n",
                "\n",
                "nothing open\n"
            )
        );
    }

    #[test]
    fn a_check_and_a_note_take_their_own_lines() {
        let mut proposal = proposal();
        let comment = note(&proposal);
        proposal
            .apply(Record::Check(Check::new(
                CheckName::parse("gate").unwrap(),
                CheckStatus::Passed,
                proposal.head().sha().clone(),
                7,
                Author::parse("ada").unwrap(),
                Timestamp::from_unix(1_760_000_050),
            )))
            .unwrap();
        proposal.apply(Record::Comment(comment.clone())).unwrap();

        let text = show(&proposal);

        assert!(text.contains("gate             passed in 7s\n\n"), "{text}");
        assert!(
            text.ends_with(&format!("{}  src/main.rs:12:14\n  say why\n", comment.id())),
            "{text}"
        );
    }

    #[test]
    fn a_thread_reads_one_way_for_a_person_and_another_for_an_agent() {
        let mut proposal = proposal();
        let comment = note(&proposal);
        proposal.apply(Record::Comment(comment.clone())).unwrap();

        assert_eq!(
            comments(&proposal, Scope::Open, Shape::Text),
            format!("{}  src/main.rs:12  say why\n", comment.id())
        );
        assert_eq!(
            comments(&proposal, Scope::Open, Shape::Json),
            format!("{}\n", Record::Comment(comment).to_line())
        );
    }

    #[test]
    fn a_result_reads_as_a_column() {
        let line = check_line(&Check::new(
            CheckName::parse("gate").unwrap(),
            CheckStatus::Failed,
            sha('a'),
            12,
            Author::parse("ada").unwrap(),
            Timestamp::from_unix(1_760_000_050),
        ));

        assert_eq!(line, "gate             failed  12s");
    }
}
