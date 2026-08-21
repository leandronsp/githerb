package review

import (
	"fmt"
	"regexp"
	"strings"
	"time"
)

// SHA is a full commit object name.
type SHA string

// File is a path inside the repository.
type File string

// ID identifies a record, and is derived from the record's own content, so the
// same annotation written twice is one annotation and the append-only log
// deduplicates itself.
type ID string

// Kind names what a record says.
type Kind string

// The kinds of record the log carries.
const (
	KindComment Kind = "comment"
	KindResolve Kind = "resolve"
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

// Record is one line of the log. Exactly one of its shapes is present, and the
// two-value accessors are the only way to reach them.
type Record struct {
	kind       Kind
	comment    Comment
	resolution Resolution
}

// The absent half of a record. Naming them says the omission is deliberate.
var (
	noComment    Comment
	noResolution Resolution
)

// CommentRecord wraps a comment as a log record.
func CommentRecord(comment Comment) Record {
	return Record{kind: KindComment, comment: comment, resolution: noResolution}
}

// ResolutionRecord wraps a resolution as a log record.
func ResolutionRecord(resolution Resolution) Record {
	return Record{kind: KindResolve, comment: noComment, resolution: resolution}
}

// Kind says which shape the record carries.
func (r Record) Kind() Kind { return r.kind }

// Comment returns the comment, and false when the record is not one.
func (r Record) Comment() (Comment, bool) {
	return r.comment, r.kind == KindComment
}

// Resolution returns the resolution, and false when the record is not one.
func (r Record) Resolution() (Resolution, bool) {
	return r.resolution, r.kind == KindResolve
}
