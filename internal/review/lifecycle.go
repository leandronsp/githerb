package review

import (
	"encoding/json"
	"fmt"
	"strings"
	"time"
)

// EventKind names a moment in a proposal's life.
type EventKind string

// The moments a proposal has. Everything between them is annotation.
const (
	EventOpened     EventKind = "opened"
	EventLanded     EventKind = "landed"
	EventAbandoned  EventKind = "abandoned"
	EventRetargeted EventKind = "retargeted"
)

// Event is one line of the proposal log: it opened, or it landed. Like every
// other record here it is appended and never edited, so the state of a
// proposal is what its events add up to rather than a field someone rewrote.
type Event struct {
	kind   EventKind
	id     ProposalID
	title  string
	target Branch
	base   SHA
	author string
	at     time.Time
}

// Opened is the event that starts a proposal.
func Opened(id ProposalID, title string, target Branch, base SHA, author string, at time.Time) (Event, error) {
	title = strings.TrimSpace(title)
	author = strings.TrimSpace(author)

	switch {
	case strings.TrimSpace(string(id)) == "":
		return Event{}, ErrNoProposalID
	case title == "":
		return Event{}, ErrNoTitle
	case strings.TrimSpace(string(target)) == "":
		return Event{}, ErrNoBranch
	case !shaPattern.MatchString(string(base)):
		return Event{}, fmt.Errorf("base %q: %w", base, ErrNoRevision)
	case author == "":
		return Event{}, ErrNoAuthor
	}

	return Event{
		kind:   EventOpened,
		id:     id,
		title:  title,
		target: target,
		base:   base,
		author: author,
		at:     at.UTC().Truncate(time.Second),
	}, nil
}

// Retargeted is the event for a proposal that lands somewhere else now. It
// happens when the branch underneath it lands: a stack is a chain of proposals
// aimed at each other, and the one on top has to follow.
func Retargeted(id ProposalID, target Branch, author string, at time.Time) (Event, error) {
	author = strings.TrimSpace(author)

	switch {
	case strings.TrimSpace(string(id)) == "":
		return Event{}, ErrNoProposalID
	case strings.TrimSpace(string(target)) == "":
		return Event{}, ErrNoBranch
	case author == "":
		return Event{}, ErrNoAuthor
	}

	return Event{
		kind:   EventRetargeted,
		id:     id,
		title:  "",
		target: target,
		base:   "",
		author: author,
		at:     at.UTC().Truncate(time.Second),
	}, nil
}

// Abandoned is the event for a proposal that will not be landing.
func Abandoned(id ProposalID, author string, at time.Time) (Event, error) {
	return ending(EventAbandoned, id, author, at)
}

// Landed is the event that ends one.
func Landed(id ProposalID, author string, at time.Time) (Event, error) {
	return ending(EventLanded, id, author, at)
}

func ending(kind EventKind, id ProposalID, author string, at time.Time) (Event, error) {
	author = strings.TrimSpace(author)

	switch {
	case strings.TrimSpace(string(id)) == "":
		return Event{}, ErrNoProposalID
	case author == "":
		return Event{}, ErrNoAuthor
	}

	return Event{
		kind:   kind,
		id:     id,
		title:  "",
		target: "",
		base:   "",
		author: author,
		at:     at.UTC().Truncate(time.Second),
	}, nil
}

// Kind says which moment this is.
func (e Event) Kind() EventKind { return e.kind }

// ID names the proposal the event belongs to.
func (e Event) ID() ProposalID { return e.id }

// Title is the proposal's title, on an opened event.
func (e Event) Title() string { return e.title }

// Target is the branch it lands on, on an opened event.
func (e Event) Target() Branch { return e.target }

// Base is the commit it was cut from, on an opened event.
func (e Event) Base() SHA { return e.base }

// Author is who caused the event.
func (e Event) Author() string { return e.author }

// At is when, to the second, in UTC.
func (e Event) At() time.Time { return e.at }

type eventLine struct {
	Version int       `json:"v"`
	Kind    EventKind `json:"kind"`
	ID      string    `json:"id"`
	Title   string    `json:"title,omitempty"`
	Target  string    `json:"target,omitempty"`
	Base    string    `json:"base,omitempty"`
	Author  string    `json:"author"`
	At      string    `json:"at"`
}

// MarshalLine renders the event as the single line it is stored as.
func (e Event) MarshalLine() ([]byte, error) {
	raw, err := json.Marshal(eventLine{
		Version: version,
		Kind:    e.kind,
		ID:      string(e.id),
		Title:   e.title,
		Target:  string(e.target),
		Base:    string(e.base),
		Author:  e.author,
		At:      e.at.Format(time.RFC3339),
	})
	if err != nil {
		return nil, fmt.Errorf("rendering an event: %w", err)
	}

	return raw, nil
}

// ParseEvent reads one line of the proposal log.
func ParseEvent(raw []byte) (Event, error) {
	var l eventLine

	if err := json.Unmarshal(raw, &l); err != nil {
		return Event{}, fmt.Errorf("%q: %w", raw, ErrMalformed)
	}

	if l.Version != version {
		return Event{}, fmt.Errorf("version %d: %w", l.Version, ErrVersion)
	}

	at, err := time.Parse(time.RFC3339, l.At)
	if err != nil {
		return Event{}, fmt.Errorf("timestamp %q: %w", l.At, ErrMalformed)
	}

	switch l.Kind {
	case EventOpened:
		return Opened(ProposalID(l.ID), l.Title, Branch(l.Target), SHA(l.Base), l.Author, at)
	case EventLanded:
		return Landed(ProposalID(l.ID), l.Author, at)
	case EventAbandoned:
		return Abandoned(ProposalID(l.ID), l.Author, at)
	case EventRetargeted:
		return Retargeted(ProposalID(l.ID), Branch(l.Target), l.Author, at)
	default:
		return Event{}, fmt.Errorf("%q: %w", l.Kind, ErrUnknownKind)
	}
}
