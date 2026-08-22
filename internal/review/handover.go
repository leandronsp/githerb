package review

import (
	"fmt"
	"strings"
)

// Handover is the whole review as one instruction an agent can act on: every
// open note, where it points, and the command that answers it. A reviewer
// leaves notes for an hour and hands them over once, rather than relaying them
// one at a time.
//
// It is empty when nothing is open, because there is nothing to say.
func (p Proposal) Handover() string {
	return p.brief(true, fmt.Sprintf("Apply each note and resolve it, then: githerb revise %s\n", p.id))
}

// Brief is the same notes handed to an agent by a runner, which records the
// revision itself. An agent told to record it too records it first, and then
// the runner is the one that looks like it failed.
func (p Proposal) Brief() string {
	return "Answer every note below, in words, by running this once per note:\n\n" +
		`  printf '%s\n' '{"note":"<id>","say":"<one line, plain, no markdown>"}' >> "$GITHERB_ANSWERS"` +
		"\n\nThat is the only way anything you say reaches the person who asked. A note you\n" +
		"answered by changing code still gets a line saying what you changed.\n\n" +
		p.decisions() + p.brief(false,
		"Then, for the notes that asked for a change, make it here and commit.\n"+
			"Do not push, do not rebase, and do not run githerb: the commit you leave here\n"+
			"is read back as the next revision.\n")
}

// decisions is what the proposal already settled. The agent answering the
// notes is a new process with no memory of the one that wrote the code, so the
// reasoning travels with the work or it gets re-litigated every revision.
func (p Proposal) decisions() string {
	if len(p.chunks) == 0 || len(p.Open()) == 0 {
		return ""
	}

	var b strings.Builder

	b.WriteString("What this proposal already decided, and is not up for debate:\n")

	for _, chunk := range p.chunks {
		fmt.Fprintf(&b, "- %s: %s\n", chunk.title, chunk.decision)
	}

	b.WriteString("\n")

	return b.String()
}

func (p Proposal) brief(commands bool, closing string) string {
	open := p.Open()
	if len(open) == 0 {
		return ""
	}

	var b strings.Builder

	fmt.Fprintf(&b, "Review of %s onto %s, revision %d. %d %s to apply.\n",
		p.id, p.target, p.Head().Number(), len(open), plural(len(open), "note"))

	for _, comment := range open {
		span := comment.Span()

		fmt.Fprintf(&b, "\n%s:%d", comment.File(), span.Start())

		if span.Lines() > 1 {
			fmt.Fprintf(&b, "-%d", span.End())
		}

		fmt.Fprintf(&b, " %s  [note %s]\n  %s\n", span.Side(), comment.ID(), comment.Body())

		if commands {
			fmt.Fprintf(&b, "  githerb resolve %s %s\n", p.id, comment.ID())
		}
	}

	b.WriteString("\n" + closing)

	return b.String()
}

func plural(n int, word string) string {
	if n == 1 {
		return word
	}

	return word + "s"
}
