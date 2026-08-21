package app

import (
	"fmt"
	"regexp"
	"strings"
	"time"

	"github.com/leandronsp/githerb/internal/review"
)

// Clock is where the time comes from, so a use case stays testable.
type Clock func() time.Time

// Propose opens a proposal for the work between a branch and a commit.
type Propose struct {
	Proposals review.Proposals
	Git       review.Git
	Author    string
	Now       Clock
}

// Run opens the proposal and returns it.
func (p Propose) Run(title, target, head string) (review.Proposal, error) {
	branch, err := review.ParseBranch(target)
	if err != nil {
		return review.Proposal{}, err
	}

	tip, err := p.Git.HeadOf(branch)
	if err != nil {
		return review.Proposal{}, fmt.Errorf("branch %s: %w", branch, err)
	}

	revision, err := p.Git.Resolve(head)
	if err != nil {
		return review.Proposal{}, fmt.Errorf("revision %s: %w", head, err)
	}

	base, err := p.Git.MergeBase(tip, revision)
	if err != nil {
		return review.Proposal{}, err
	}

	proposal, err := review.NewProposal(slug(title, revision), title, branch, base, revision)
	if err != nil {
		return review.Proposal{}, err
	}

	opened, err := review.Opened(proposal.ID(), title, branch, base, p.Author, p.Now())
	if err != nil {
		return review.Proposal{}, err
	}

	if err := p.Proposals.Open(proposal, opened); err != nil {
		return review.Proposal{}, err
	}

	return proposal, nil
}

var notSlug = regexp.MustCompile(`[^a-z0-9]+`)

// slug names a proposal after its title, with a piece of the commit on the end
// so two proposals with the same title never collide.
func slug(title string, head review.SHA) review.ProposalID {
	name := strings.Trim(notSlug.ReplaceAllString(strings.ToLower(title), "-"), "-")

	if len(name) > 40 {
		name = strings.Trim(name[:40], "-")
	}

	if name == "" {
		name = "proposal"
	}

	return review.ProposalID(name + "-" + string(head)[:7])
}
