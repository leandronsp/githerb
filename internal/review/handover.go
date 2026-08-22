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
	return p.brief(false, "Apply every note above in this working directory and commit.\n"+
		"Do not push, do not rebase, and do not run githerb: the commit you leave\n"+
		"here is read back as the next revision.\n")
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

		fmt.Fprintf(&b, " %s\n  %s\n", span.Side(), comment.Body())

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
