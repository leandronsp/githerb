//! The review as one instruction, in the exact words an agent reads on stdin.
//!
//! A reviewer leaves notes for an hour and hands them over once, rather than
//! relaying them one at a time. Both texts are built from the conversation and
//! not from the blocking set: a question that fell off the head when somebody
//! committed is still a question nobody answered.
//!
//! Both are empty when the conversation is empty, because there is nothing to
//! say.

use std::fmt::Write as _;

use crate::identity::{ProposalId, Sha};
use crate::proposal::Proposal;

/// Whether the brief tells the reader which command answers a note.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Commands {
    /// A person is reading, and will run `githerb resolve` themselves.
    Shown,
    /// A runner is reading, and records the revision itself.
    Hidden,
}

/// What a person copies from the browser button, and what `githerb handover`
/// prints.
#[must_use]
pub fn handover(proposal: &Proposal) -> String {
    let closing = format!(
        "Apply each note and resolve it, then: githerb revise {}\n",
        proposal.id()
    );
    body(proposal, Commands::Shown, &closing)
}

/// The same notes handed to an agent by a runner, which records the revision
/// itself. An agent told to record it too records it first, and then the
/// runner is the one that looks like it failed.
#[must_use]
pub fn brief(proposal: &Proposal) -> String {
    let notes = body(
        proposal,
        Commands::Hidden,
        "Then, for the notes that asked for a change, make it here and commit.\n\
         Do not push, do not rebase, and do not run githerb: the commit you leave here\n\
         is read back as the next revision.\n",
    );
    if notes.is_empty() {
        return notes;
    }
    let preamble = concat!(
        "Answer every note below, in words, by running this once per note:\n\n",
        r#"  printf '%s\n' '{"note":"<id>","say":"<one line, plain, no markdown>"}' >> "$GITHERB_ANSWERS""#,
        "\n\nThat is the only way anything you say reaches the person who asked. A note you\n",
        "answered by changing code still gets a line saying what you changed.\n\n",
    );
    format!("{preamble}{}{notes}", decisions(proposal))
}

/// What a rebase that stopped hands to an agent. It is not a review: nothing
/// is open, there is only a conflict somebody has to resolve.
#[must_use]
pub fn conflict_brief(id: &ProposalId, onto: &Sha) -> String {
    format!(
        "A rebase of {id} onto {} stopped on a conflict.\n\
         Resolve every conflict in this worktree, keeping what the proposal is for, then\n\
         git add the files and run git rebase --continue until the rebase is finished.\n\
         Change nothing else.\n",
        onto.short()
    )
}

/// What the proposal already settled.
///
/// The agent answering the notes is a new process with no memory of the one
/// that wrote the code, so the reasoning travels with the work or it gets
/// re-litigated every revision.
fn decisions(proposal: &Proposal) -> String {
    if proposal.chunks().is_empty() || proposal.open_comments().is_empty() {
        return String::new();
    }
    let mut text = String::from("What this proposal already decided, and is not up for debate:\n");
    for chunk in proposal.chunks() {
        let _ = writeln!(text, "- {}: {}", chunk.title(), chunk.decision());
    }
    text.push('\n');
    text
}

fn body(proposal: &Proposal, commands: Commands, closing: &str) -> String {
    let open = proposal.conversation();
    if open.is_empty() {
        return String::new();
    }

    let mut text = String::new();
    let _ = writeln!(
        text,
        "Review of {} onto {}, revision {}. {} {} to apply.",
        proposal.id(),
        proposal.target(),
        proposal.head().number(),
        open.len(),
        plural(open.len(), "note"),
    );

    for comment in open {
        let _ = write!(
            text,
            "\n{}  [note {}]\n  {}\n",
            comment.anchor(),
            comment.id(),
            comment.body()
        );
        // What was already said under it, so the answer continues the thread
        // instead of starting it again.
        for answer in proposal.answers(comment.id()) {
            let _ = writeln!(text, "    {}: {}", answer.author(), answer.body());
        }
        if commands == Commands::Shown {
            let _ = writeln!(text, "  githerb resolve {} {}", proposal.id(), comment.id());
        }
    }

    text.push('\n');
    text.push_str(closing);
    text
}

fn plural(count: usize, word: &str) -> String {
    if count == 1 {
        word.to_owned()
    } else {
        format!("{word}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::branch::Branch;
    use crate::chunk::Chunk;
    use crate::comment::Comment;
    use crate::errors::Result;
    use crate::fixtures::{anchor, at, author};
    use crate::record::Record;
    use crate::reply::Reply;
    use crate::resolution::Resolution;
    use crate::timestamp::Timestamp;

    const HEAD: &str = "9f6c1e2a3b4d5e6f708192a3b4c5d6e7f8091a2b";
    const BASE: &str = "00112233445566778899aabbccddeeff00112233";
    const MOMENT: i64 = 1_787_335_445;

    const HANDOVER: &str = r"Review of land-the-gate onto main, revision 1. 1 note to apply.

internal/app/land.go:42-47 new  [note 9b052da286a4]
  this leaks the handle when init fails
    claude-code: renamed it
  githerb resolve land-the-gate 9b052da286a4

Apply each note and resolve it, then: githerb revise land-the-gate
";

    const BRIEF: &str = r#"Answer every note below, in words, by running this once per note:

  printf '%s\n' '{"note":"<id>","say":"<one line, plain, no markdown>"}' >> "$GITHERB_ANSWERS"

That is the only way anything you say reaches the person who asked. A note you
answered by changing code still gets a line saying what you changed.

What this proposal already decided, and is not up for debate:
- Checks appear where you already look: one column in list

Review of land-the-gate onto main, revision 1. 1 note to apply.

internal/app/land.go:42-47 new  [note 9b052da286a4]
  this leaks the handle when init fails
    claude-code: renamed it

Then, for the notes that asked for a change, make it here and commit.
Do not push, do not rebase, and do not run githerb: the commit you leave here
is read back as the next revision.
"#;

    fn reviewed() -> Result<Proposal> {
        let mut proposal = Proposal::open(
            ProposalId::parse("land-the-gate")?,
            "Land the gate",
            Branch::parse("main")?,
            Sha::parse(BASE)?,
            Sha::parse(HEAD)?,
            at(MOMENT),
        )?;
        let note = Comment::new(
            Sha::parse(HEAD)?,
            anchor("internal/app/land.go"),
            "this leaks the handle when init fails",
            author("leandro"),
            at(MOMENT),
        )?;
        let answer = Reply::new(
            note.id().clone(),
            Sha::parse(HEAD)?,
            "renamed it",
            author("claude-code"),
            at(MOMENT),
        )?;
        proposal.fold([
            Record::Chunk(Chunk::new(
                "Checks appear where you already look",
                Some("cmd/githerb"),
                "you had to open the browser",
                "list carries a column",
                "one column in list",
                None,
            )?),
            Record::Comment(note),
            Record::Reply(answer),
        ])?;
        Ok(proposal)
    }

    #[test]
    fn a_note_reads_the_same_way_it_did_before_the_rewrite() -> Result<()> {
        assert_eq!(handover(&reviewed()?), HANDOVER);
        Ok(())
    }

    #[test]
    fn the_runner_brief_is_the_notes_without_the_commands() -> Result<()> {
        let proposal = reviewed()?;
        let brief = brief(&proposal);
        assert_eq!(brief, BRIEF);
        assert!(!brief.contains("githerb revise"));
        assert!(!brief.contains("githerb resolve"));
        assert!(brief.contains("this leaks the handle when init fails"));
        assert_ne!(brief, handover(&proposal));
        Ok(())
    }

    #[test]
    fn a_proposal_with_nothing_open_hands_over_nothing() -> Result<()> {
        let mut proposal = reviewed()?;
        let note = proposal
            .conversation()
            .first()
            .map(|comment| comment.id().clone())
            .ok_or(crate::errors::Error::NoBody)?;
        proposal.apply(Record::Resolve(Resolution::new(
            note,
            author("leandro"),
            at(MOMENT),
        )))?;
        assert_eq!(handover(&proposal), "");
        assert_eq!(brief(&proposal), "");
        Ok(())
    }

    #[test]
    fn a_note_that_fell_off_the_head_is_still_handed_over() -> Result<()> {
        let mut proposal = reviewed()?;
        proposal.add_revision(Sha::parse("11".repeat(20).as_str())?)?;
        assert_eq!(proposal.open_comments().len(), 0);
        assert!(handover(&proposal).contains("[note 9b052da286a4]"));
        Ok(())
    }

    #[test]
    fn the_decisions_block_is_dropped_once_nothing_is_open_on_the_head() -> Result<()> {
        let mut proposal = reviewed()?;
        proposal.add_revision(Sha::parse("11".repeat(20).as_str())?)?;
        let brief = brief(&proposal);
        assert!(!brief.contains("is not up for debate"));
        assert!(brief.contains("[note 9b052da286a4]"));
        Ok(())
    }

    #[test]
    fn two_notes_are_notes_and_one_note_is_a_note() -> Result<()> {
        let mut proposal = reviewed()?;
        proposal.apply(Record::Comment(Comment::new(
            Sha::parse(HEAD)?,
            anchor("internal/app/land.go"),
            "and this one shadows the error",
            author("leandro"),
            at(MOMENT),
        )?))?;
        assert!(handover(&proposal).contains("2 notes to apply."));
        Ok(())
    }

    #[test]
    fn a_conflict_hands_over_the_worktree_and_nothing_else() -> Result<()> {
        assert_eq!(
            conflict_brief(&ProposalId::parse("land-the-gate")?, &Sha::parse(HEAD)?),
            "A rebase of land-the-gate onto 9f6c1e2 stopped on a conflict.\n\
             Resolve every conflict in this worktree, keeping what the proposal is for, then\n\
             git add the files and run git rebase --continue until the rebase is finished.\n\
             Change nothing else.\n"
        );
        Ok(())
    }

    #[test]
    fn a_single_line_note_says_one_line_and_not_a_range() -> Result<()> {
        let mut proposal = Proposal::open(
            ProposalId::parse("gate")?,
            "Gate",
            Branch::parse("main")?,
            Sha::parse(BASE)?,
            Sha::parse(HEAD)?,
            Timestamp::from_unix(0),
        )?;
        proposal.apply(Record::Comment(Comment::new(
            Sha::parse(HEAD)?,
            crate::span::Anchor::new(
                crate::identity::FilePath::parse("a.go")?,
                crate::span::Span::new(crate::span::Side::Old, 7, 7)?,
            ),
            "here",
            author("leandro"),
            Timestamp::from_unix(0),
        )?))?;
        assert!(handover(&proposal).contains("\na.go:7 old  [note "));
        Ok(())
    }
}
