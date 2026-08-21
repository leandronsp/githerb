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

		fmt.Fprintf(&b, " %s\n  %s\n  githerb resolve %s %s\n", span.Side(), comment.Body(), p.id, comment.ID())
	}

	fmt.Fprintf(&b, "\nApply each note and resolve it, then: githerb revise %s\n", p.id)

	return b.String()
}

func plural(n int, word string) string {
	if n == 1 {
		return word
	}

	return word + "s"
}
