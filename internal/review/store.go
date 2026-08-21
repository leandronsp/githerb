package review

// Proposals is how proposals are kept. The domain declares what it needs and
// an adapter happens to satisfy it, so nothing in this package knows that the
// answer is refs and notes.
//
// Every method is an intention rather than a row operation, because the store
// is append-only underneath and "update this proposal" is not a thing that can
// honestly happen.
type Proposals interface {
	// Open records a new proposal and its first revision.
	Open(proposal Proposal, event Event) error

	// Load rebuilds a proposal from everything written about it.
	Load(id ProposalID) (Proposal, error)

	// List rebuilds every proposal, newest first.
	List() ([]Proposal, error)

	// Revise records another attempt at an open proposal.
	Revise(id ProposalID, revision Revision) error

	// Annotate appends one record to the log of a revision.
	Annotate(revision SHA, record Record) error

	// Land moves the target branch onto the proposal's head and records it.
	Land(proposal Proposal, event Event) error
}

// Git is the small part of git the application needs beyond storage: naming
// commits and showing what changed between them.
type Git interface {
	// Resolve turns anything git accepts as a revision into a commit.
	Resolve(revision string) (SHA, error)

	// HeadOf is the commit a branch points at.
	HeadOf(branch Branch) (SHA, error)

	// MergeBase is the commit two revisions last had in common.
	MergeBase(one, other SHA) (SHA, error)

	// Diff is the patch between two commits.
	Diff(from, to SHA) (string, error)
}
