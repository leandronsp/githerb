package review

import (
	"strings"
	"time"
)

// Reply answers a note. It carries no file and no lines of its own: it belongs
// to the thread of the note it answers, which is where it is read.
//
// A reply never blocks a landing. The question is what blocks, and the person
// who asked it is the one who decides it was answered.
type Reply struct {
	id       ID
	target   ID
	revision SHA
	body     string
	author   string
	at       time.Time
}

// NewReply is the only way to build one.
func NewReply(target ID, revision SHA, body, author string, at time.Time) (Reply, error) {
	body = strings.TrimSpace(body)
	author = strings.TrimSpace(author)

	switch {
	case strings.TrimSpace(string(target)) == "":
		return Reply{}, ErrNoTarget
	case !shaPattern.MatchString(string(revision)):
		return Reply{}, ErrNoRevision
	case body == "":
		return Reply{}, ErrNoBody
	case author == "":
		return Reply{}, ErrNoAuthor
	}

	made := Reply{
		id: "", target: target, revision: revision,
		body: body, author: author, at: at.UTC().Truncate(time.Second),
	}

	return made.identified()
}

// ID names the reply, derived from what it says.
func (r Reply) ID() ID { return r.id }

// Target is the note it answers.
func (r Reply) Target() ID { return r.target }

// Revision is the head it was written against.
func (r Reply) Revision() SHA { return r.revision }

// Body is what it says.
func (r Reply) Body() string { return r.body }

// Author is who said it, a person or an agent.
func (r Reply) Author() string { return r.author }

// At is when, to the second, in UTC.
func (r Reply) At() time.Time { return r.at }
