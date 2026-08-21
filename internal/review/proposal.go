package review

import (
	"fmt"
	"strings"
)

// Proposal is the aggregate root. Every rule that spans more than one value
// lives here: a comment belongs to a revision of this proposal, a resolution
// answers a comment this proposal has seen, and landing is refused while the
// head revision still has something open.
//
// It is a value. Every change returns a new proposal and leaves the old one
// alone, which is what lets the log be folded without the order of two
// concurrent readers mattering.
type Proposal struct {
	id        ProposalID
	title     string
	target    Branch
	base      SHA
	state     State
	revisions []Revision
	comments  []Comment
	resolved  map[ID]bool
}

// NewProposal opens a proposal at its first revision. The target is whichever
// branch this is meant to land on, which is often the trunk and need not be.
func NewProposal(id ProposalID, title string, target Branch, base, head SHA) (Proposal, error) {
	title = strings.TrimSpace(title)

	switch {
	case strings.TrimSpace(string(id)) == "":
		return Proposal{}, ErrNoProposalID
	case title == "":
		return Proposal{}, ErrNoTitle
	case strings.TrimSpace(string(target)) == "":
		return Proposal{}, ErrNoBranch
	case !shaPattern.MatchString(string(base)):
		return Proposal{}, fmt.Errorf("base %q: %w", base, ErrNoRevision)
	case !shaPattern.MatchString(string(head)):
		return Proposal{}, fmt.Errorf("head %q: %w", head, ErrNoRevision)
	case base == head:
		return Proposal{}, ErrNothingProposed
	}

	return Proposal{
		id:        id,
		title:     title,
		target:    target,
		base:      base,
		state:     StateOpen,
		revisions: []Revision{{number: 1, sha: head}},
		comments:  nil,
		resolved:  map[ID]bool{},
	}, nil
}

// ID names the proposal.
func (p Proposal) ID() ProposalID { return p.id }

// Title is what a human calls it.
func (p Proposal) Title() string { return p.title }

// Target is the branch the proposal lands on.
func (p Proposal) Target() Branch { return p.target }

// Base is the commit the proposal was cut from.
func (p Proposal) Base() SHA { return p.base }

// State is where the proposal is in its life.
func (p Proposal) State() State { return p.state }

// Revisions are every attempt, oldest first.
func (p Proposal) Revisions() []Revision {
	out := make([]Revision, len(p.revisions))
	copy(out, p.revisions)

	return out
}

// Head is the latest revision, the one being reviewed.
func (p Proposal) Head() Revision { return p.revisions[len(p.revisions)-1] }

// WithRevision adds the commit an agent produced after reading the annotations.
func (p Proposal) WithRevision(sha SHA) (Proposal, error) {
	if p.state != StateOpen {
		return Proposal{}, fmt.Errorf("proposal is %s: %w", p.state, ErrNotOpen)
	}

	if !shaPattern.MatchString(string(sha)) {
		return Proposal{}, fmt.Errorf("%q: %w", sha, ErrNoRevision)
	}

	if p.knows(sha) {
		return Proposal{}, fmt.Errorf("%q: %w", sha, ErrRevisionKnown)
	}

	next := p.clone()
	next.revisions = append(next.revisions, Revision{number: len(p.revisions) + 1, sha: sha})

	return next, nil
}

// WithRecord folds one line of the log into the proposal. Applying the same
// record twice changes nothing, because the log may honestly deliver it twice.
func (p Proposal) WithRecord(record Record) (Proposal, error) {
	if p.state != StateOpen {
		return Proposal{}, fmt.Errorf("proposal is %s: %w", p.state, ErrNotOpen)
	}

	switch record.Kind() {
	case KindComment:
		comment, _ := record.Comment()

		return p.withComment(comment)
	case KindResolve:
		resolution, _ := record.Resolution()

		return p.withResolution(resolution)
	default:
		return Proposal{}, fmt.Errorf("%q: %w", record.Kind(), ErrUnknownKind)
	}
}

// Open are the comments on the head revision that nobody has resolved. They
// are what stands between the proposal and the trunk.
func (p Proposal) Open() []Comment {
	head := p.Head().sha

	var open []Comment

	for _, comment := range p.comments {
		if comment.revision == head && !p.resolved[comment.id] {
			open = append(open, comment)
		}
	}

	return open
}

// Landable reports why the proposal cannot land, and nil when it can.
func (p Proposal) Landable() error {
	if p.state != StateOpen {
		return fmt.Errorf("proposal is %s: %w", p.state, ErrNotOpen)
	}

	if open := len(p.Open()); open > 0 {
		return fmt.Errorf("%d on revision %d: %w", open, p.Head().number, ErrOpenComments)
	}

	return nil
}

// Landed records that the head revision reached the trunk.
func (p Proposal) Landed() (Proposal, error) {
	if err := p.Landable(); err != nil {
		return Proposal{}, err
	}

	next := p.clone()
	next.state = StateLanded

	return next, nil
}

func (p Proposal) withComment(comment Comment) (Proposal, error) {
	if !p.knows(comment.revision) {
		return Proposal{}, fmt.Errorf("%q: %w", comment.revision, ErrUnknownRevision)
	}

	if p.seen(comment.id) {
		return p, nil
	}

	next := p.clone()
	next.comments = append(next.comments, comment)

	return next, nil
}

func (p Proposal) withResolution(resolution Resolution) (Proposal, error) {
	if !p.seen(resolution.target) {
		return Proposal{}, fmt.Errorf("%q: %w", resolution.target, ErrUnknownComment)
	}

	next := p.clone()
	next.resolved[resolution.target] = true

	return next, nil
}

func (p Proposal) knows(sha SHA) bool {
	for _, revision := range p.revisions {
		if revision.sha == sha {
			return true
		}
	}

	return false
}

func (p Proposal) seen(id ID) bool {
	for _, comment := range p.comments {
		if comment.id == id {
			return true
		}
	}

	return false
}

// clone copies what a change would otherwise share with the proposal it came
// from, so a value really behaves like one.
func (p Proposal) clone() Proposal {
	revisions := make([]Revision, len(p.revisions))
	copy(revisions, p.revisions)

	comments := make([]Comment, len(p.comments))
	copy(comments, p.comments)

	resolved := make(map[ID]bool, len(p.resolved))
	for id, was := range p.resolved {
		resolved[id] = was
	}

	return Proposal{
		id:        p.id,
		title:     p.title,
		target:    p.target,
		base:      p.base,
		state:     p.state,
		revisions: revisions,
		comments:  comments,
		resolved:  resolved,
	}
}
