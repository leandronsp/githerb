package review

import (
	"fmt"
	"sort"
	"strings"
	"time"
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
	checks    map[CheckName]Check
	chunks    []Chunk
	rationale []Comment
	work      []Work
	asks      []Dispatch
	answers   []Reply
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
		checks:    map[CheckName]Check{},
		chunks:    nil,
		rationale: nil,
		work:      nil,
		asks:      nil,
		answers:   nil,
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
	case KindCheck:
		check, _ := record.Check()

		return p.withCheck(check)
	case KindChunk:
		chunk, _ := record.Chunk()

		next := p.clone()
		next.chunks = append(next.chunks, chunk)

		return next, nil
	case KindRationale:
		comment, _ := record.Comment()

		return p.withRationale(comment)
	case KindWork:
		work, _ := record.Work()

		next := p.clone()
		next.work = append(next.work, work)

		return next, nil
	case KindDispatch:
		dispatch, _ := record.Dispatch()

		next := p.clone()
		next.asks = append(next.asks, dispatch)

		return next, nil
	case KindReply:
		reply, _ := record.Reply()

		return p.withReply(reply)
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

// Landable reports why the proposal cannot land, and nil when it can. The
// required checks are the ones the repository declares; a proposal that
// declares none is gated only by the review.
func (p Proposal) Landable(required ...CheckName) error {
	if p.state != StateOpen {
		return fmt.Errorf("proposal is %s: %w", p.state, ErrNotOpen)
	}

	if open := len(p.Open()); open > 0 {
		return fmt.Errorf("%d on revision %d: %w", open, p.Head().number, ErrOpenComments)
	}

	current := p.Checks()

	for _, name := range required {
		check, ran := current[name]

		switch {
		case !ran:
			return fmt.Errorf("%q: %w", name, ErrCheckMissing)
		case !check.Passed():
			return fmt.Errorf("%q: %w", name, ErrCheckFailed)
		}
	}

	return nil
}

// Landed records that the head revision reached the target branch.
func (p Proposal) Landed(required ...CheckName) (Proposal, error) {
	if err := p.Landable(required...); err != nil {
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

// Checks are the results recorded against the head revision, by name. An older
// revision's result is not carried forward, because it ran on other code.
func (p Proposal) Checks() map[CheckName]Check {
	head := p.Head().sha
	current := map[CheckName]Check{}

	for name, check := range p.checks {
		if check.revision == head {
			current[name] = check
		}
	}

	return current
}

// SortedChecks are the results on the head revision, by name, so a list never
// shuffles between two renders of the same thing.
func (p Proposal) SortedChecks() []Check {
	current := p.Checks()

	names := make([]string, 0, len(current))
	for name := range current {
		names = append(names, string(name))
	}

	sort.Strings(names)

	checks := make([]Check, 0, len(names))
	for _, name := range names {
		checks = append(checks, current[CheckName(name)])
	}

	return checks
}

// CheckSummary is the shortest true thing that can be said about the checks on
// the head revision, for a column in a list.
func (p Proposal) CheckSummary() string {
	current := p.Checks()
	if len(current) == 0 {
		return "no checks"
	}

	if failing := len(p.Failing()); failing > 0 {
		return fmt.Sprintf("%d failed", failing)
	}

	return "passing"
}

// Failing are the checks on the head revision that said no.
func (p Proposal) Failing() []Check {
	var failing []Check

	for _, check := range p.Checks() {
		if !check.Passed() {
			failing = append(failing, check)
		}
	}

	return failing
}

func (p Proposal) withCheck(check Check) (Proposal, error) {
	if !p.knows(check.revision) {
		return Proposal{}, fmt.Errorf("%q: %w", check.revision, ErrUnknownRevision)
	}

	next := p.clone()
	next.checks[check.name] = check

	return next, nil
}

// Retargeted points the proposal at another branch. Nothing else moves: the
// base is where the work was cut from and stays true whatever it lands on.
func (p Proposal) Retargeted(target Branch) (Proposal, error) {
	if p.state != StateOpen {
		return Proposal{}, fmt.Errorf("proposal is %s: %w", p.state, ErrNotOpen)
	}

	branch, err := ParseBranch(string(target))
	if err != nil {
		return Proposal{}, err
	}

	next := p.clone()
	next.target = branch

	return next, nil
}

// Abandoned gives up on a proposal, which is how something that did not get in
// stays visible instead of disappearing.
func (p Proposal) Abandoned() (Proposal, error) {
	if p.state != StateOpen {
		return Proposal{}, fmt.Errorf("proposal is %s: %w", p.state, ErrNotOpen)
	}

	next := p.clone()
	next.state = StateAbandoned

	return next, nil
}

// Work is every line an agent left on this proposal.
func (p Proposal) Work() []Work {
	out := make([]Work, len(p.work))
	copy(out, p.work)

	return out
}

// withReply files an answer under the note it answers, which has to be a note
// this proposal carries.
func (p Proposal) withReply(reply Reply) (Proposal, error) {
	if !p.seen(reply.target) {
		return Proposal{}, fmt.Errorf("%q: %w", reply.target, ErrUnknownComment)
	}

	if p.seen(reply.id) {
		return p, nil
	}

	next := p.clone()
	next.answers = append(next.answers, reply)

	return next, nil
}

// Answers are the replies to a note, oldest first, which is the thread.
func (p Proposal) Answers(note ID) []Reply {
	var thread []Reply

	for _, reply := range p.answers {
		if reply.target == note {
			thread = append(thread, reply)
		}
	}

	sort.SliceStable(thread, func(i, j int) bool { return thread[i].at.Before(thread[j].at) })

	return thread
}

// Dispatched reports whether the head revision is waiting for an agent: a
// person handed it over and nothing has picked it up since.
func (p Proposal) Dispatched() bool {
	head := p.Head().sha

	var asked time.Time

	for _, ask := range p.asks {
		if ask.revision == head && ask.at.After(asked) {
			asked = ask.at
		}
	}

	if asked.IsZero() {
		return false
	}

	for _, line := range p.work {
		if line.revision == head && !line.at.Before(asked) {
			return false
		}
	}

	return true
}

// Activity is what the work log adds up to: idle, working, or stopped.
func (p Proposal) Activity() Activity { return activityOf(p.work) }

// Chunks are the decisions the author is explaining, in the order they were
// written, which is the order they should be read in.
func (p Proposal) Chunks() []Chunk {
	out := make([]Chunk, len(p.chunks))
	copy(out, p.chunks)

	return out
}

// Rationale is the author explaining some lines. It never blocks, because it
// answers a question rather than asking one.
func (p Proposal) Rationale() []Comment {
	head := p.Head().sha

	var current []Comment

	for _, comment := range p.rationale {
		if comment.revision == head {
			current = append(current, comment)
		}
	}

	return current
}

func (p Proposal) withRationale(comment Comment) (Proposal, error) {
	if !p.knows(comment.revision) {
		return Proposal{}, fmt.Errorf("%q: %w", comment.revision, ErrUnknownRevision)
	}

	for _, already := range p.rationale {
		if already.id == comment.id {
			return p, nil
		}
	}

	next := p.clone()
	next.rationale = append(next.rationale, comment)

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

	checks := make(map[CheckName]Check, len(p.checks))
	for name, check := range p.checks {
		checks[name] = check
	}

	chunks := make([]Chunk, len(p.chunks))
	copy(chunks, p.chunks)

	rationale := make([]Comment, len(p.rationale))
	copy(rationale, p.rationale)

	work := make([]Work, len(p.work))
	copy(work, p.work)

	asks := make([]Dispatch, len(p.asks))
	copy(asks, p.asks)

	answers := make([]Reply, len(p.answers))
	copy(answers, p.answers)

	return Proposal{
		id:        p.id,
		title:     p.title,
		target:    p.target,
		base:      p.base,
		state:     p.state,
		revisions: revisions,
		comments:  comments,
		resolved:  resolved,
		checks:    checks,
		chunks:    chunks,
		rationale: rationale,
		work:      work,
		asks:      asks,
		answers:   answers,
	}
}
