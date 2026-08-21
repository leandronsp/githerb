package review

import (
	"fmt"
	"regexp"
	"strings"
	"time"
)

var shaPattern = regexp.MustCompile(`\A[0-9a-f]{40}\z`)

// Comment is a human's note on a range of lines of one revision.
type Comment struct {
	id       ID
	revision SHA
	file     File
	span     Span
	body     string
	author   string
	at       time.Time
}

// NewComment is the only way to build a Comment.
func NewComment(revision SHA, file File, span Span, body, author string, at time.Time) (Comment, error) {
	body = strings.TrimSpace(body)
	author = strings.TrimSpace(author)

	switch {
	case !shaPattern.MatchString(string(revision)):
		return Comment{}, fmt.Errorf("%q: %w", revision, ErrNoRevision)
	case strings.TrimSpace(string(file)) == "":
		return Comment{}, ErrNoFile
	case body == "":
		return Comment{}, ErrNoBody
	case author == "":
		return Comment{}, ErrNoAuthor
	}

	comment := Comment{
		id:       "",
		revision: revision,
		file:     file,
		span:     span,
		body:     body,
		author:   author,
		at:       at.UTC().Truncate(time.Second),
	}

	identified, err := comment.identified()
	if err != nil {
		return Comment{}, err
	}

	return identified, nil
}

// ID is the content-derived identity of the comment.
func (c Comment) ID() ID { return c.id }

// Revision is the commit the comment applies to.
func (c Comment) Revision() SHA { return c.revision }

// File is the path the comment applies to.
func (c Comment) File() File { return c.file }

// Span is the range of lines the comment applies to.
func (c Comment) Span() Span { return c.span }

// Body is what the comment says.
func (c Comment) Body() string { return c.body }

// Author is who left the comment.
func (c Comment) Author() string { return c.author }

// At is when the comment was left, to the second, in UTC.
func (c Comment) At() time.Time { return c.at }
