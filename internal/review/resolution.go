package review

import (
	"strings"
	"time"
)

// Resolution says a comment has been dealt with. It never edits the comment,
// because the log is append-only.
type Resolution struct {
	id     ID
	target ID
	author string
	at     time.Time
}

// NewResolution is the only way to build a Resolution.
func NewResolution(target ID, author string, at time.Time) (Resolution, error) {
	author = strings.TrimSpace(author)

	switch {
	case strings.TrimSpace(string(target)) == "":
		return Resolution{}, ErrNoTarget
	case author == "":
		return Resolution{}, ErrNoAuthor
	}

	resolution := Resolution{
		id:     "",
		target: target,
		author: author,
		at:     at.UTC().Truncate(time.Second),
	}

	identified, err := resolution.identified()
	if err != nil {
		return Resolution{}, err
	}

	return identified, nil
}

// ID is the content-derived identity of the resolution.
func (r Resolution) ID() ID { return r.id }

// Target is the comment this resolution answers.
func (r Resolution) Target() ID { return r.target }

// Author is who resolved it.
func (r Resolution) Author() string { return r.author }

// At is when it was resolved, to the second, in UTC.
func (r Resolution) At() time.Time { return r.at }
