package review

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"time"
)

// version is the shape of the line an agent parses. It is a contract with
// other people's tooling, so it changes only with this number.
const version = 1

// idLength keeps an identity short enough to type and long enough not to
// collide in a repository's worth of comments.
const idLength = 12

// line is the wire shape. Field order here is the field order on disk, and the
// order is part of the contract: identical records must serialise to identical
// bytes so git's cat_sort_uniq merge deduplicates them.
type line struct {
	Version int         `json:"v"`
	Kind    Kind        `json:"kind"`
	ID      ID          `json:"id"`
	Target  ID          `json:"target,omitempty"`
	Rev     SHA         `json:"rev,omitempty"`
	File    File        `json:"file,omitempty"`
	Side    Side        `json:"side,omitempty"`
	Start   int         `json:"start,omitempty"`
	End     int         `json:"end,omitempty"`
	Body    string      `json:"body,omitempty"`
	Name    CheckName   `json:"name,omitempty"`
	Status  CheckStatus `json:"status,omitempty"`
	Seconds int         `json:"seconds,omitempty"`
	Author  string      `json:"author"`
	At      string      `json:"at"`
}

// MarshalLine renders the comment as the single line it is stored as.
func (c Comment) MarshalLine() ([]byte, error) { return marshal(c.line()) }

// MarshalLine renders the resolution as the single line it is stored as.
func (r Resolution) MarshalLine() ([]byte, error) { return marshal(r.line()) }

// MarshalLine renders the check as the single line it is stored as.
func (c Check) MarshalLine() ([]byte, error) { return marshal(c.line()) }

func (c Comment) line() line {
	return line{
		Version: version,
		Kind:    KindComment,
		ID:      c.id,
		Target:  "",
		Rev:     c.revision,
		File:    c.file,
		Side:    c.span.side,
		Start:   c.span.start,
		End:     c.span.end,
		Body:    c.body,
		Name:    "",
		Status:  "",
		Seconds: 0,
		Author:  c.author,
		At:      c.at.Format(time.RFC3339),
	}
}

func (c Check) line() line {
	return line{
		Version: version,
		Kind:    KindCheck,
		ID:      "",
		Target:  "",
		Rev:     c.revision,
		File:    "",
		Side:    "",
		Start:   0,
		End:     0,
		Body:    "",
		Name:    c.name,
		Status:  c.status,
		Seconds: c.seconds,
		Author:  c.author,
		At:      c.at.Format(time.RFC3339),
	}
}

func (r Resolution) line() line {
	return line{
		Version: version,
		Kind:    KindResolve,
		ID:      r.id,
		Target:  r.target,
		Rev:     "",
		File:    "",
		Side:    "",
		Start:   0,
		End:     0,
		Body:    "",
		Name:    "",
		Status:  "",
		Seconds: 0,
		Author:  r.author,
		At:      r.at.Format(time.RFC3339),
	}
}

func (c Comment) identified() (Comment, error) {
	id, err := derive(c.line())
	if err != nil {
		return Comment{}, err
	}

	c.id = id

	return c, nil
}

func (r Resolution) identified() (Resolution, error) {
	id, err := derive(r.line())
	if err != nil {
		return Resolution{}, err
	}

	r.id = id

	return r, nil
}

// derive hashes the record with its identity left empty, the way git names a
// blob after what is inside it.
func derive(l line) (ID, error) {
	l.ID = ""

	raw, err := marshal(l)
	if err != nil {
		return "", err
	}

	sum := sha256.Sum256(raw)

	return ID(hex.EncodeToString(sum[:])[:idLength]), nil
}

func marshal(l line) ([]byte, error) {
	raw, err := json.Marshal(l)
	if err != nil {
		return nil, fmt.Errorf("rendering a record: %w", err)
	}

	return raw, nil
}

// ParseLine reads one line of the log.
func ParseLine(raw []byte) (Record, error) {
	var l line

	if err := json.Unmarshal(raw, &l); err != nil {
		return Record{}, fmt.Errorf("%q: %w", raw, ErrMalformed)
	}

	if l.Version != version {
		return Record{}, fmt.Errorf("version %d: %w", l.Version, ErrVersion)
	}

	moment, err := time.Parse(time.RFC3339, l.At)
	if err != nil {
		return Record{}, fmt.Errorf("timestamp %q: %w", l.At, ErrMalformed)
	}

	switch l.Kind {
	case KindComment:
		return parseComment(l, moment)
	case KindResolve:
		return parseResolution(l, moment)
	case KindCheck:
		return parseCheck(l, moment)
	default:
		return Record{}, fmt.Errorf("%q: %w", l.Kind, ErrUnknownKind)
	}
}

func parseComment(l line, at time.Time) (Record, error) {
	span, err := NewSpan(l.Side, l.Start, l.End)
	if err != nil {
		return Record{}, err
	}

	comment, err := NewComment(l.Rev, l.File, span, l.Body, l.Author, at)
	if err != nil {
		return Record{}, err
	}

	return CommentRecord(comment), nil
}

func parseCheck(l line, at time.Time) (Record, error) {
	check, err := NewCheck(l.Name, l.Status, l.Rev, l.Seconds, l.Author, at)
	if err != nil {
		return Record{}, err
	}

	return CheckRecord(check), nil
}

func parseResolution(l line, at time.Time) (Record, error) {
	resolution, err := NewResolution(l.Target, l.Author, at)
	if err != nil {
		return Record{}, err
	}

	return ResolutionRecord(resolution), nil
}
