package review

import "errors"

// The reasons a record can be refused. Callers match with errors.Is.
var (
	ErrNoRevision  = errors.New("a comment must name the revision it applies to")
	ErrNoFile      = errors.New("a comment must name a file")
	ErrNoBody      = errors.New("a comment must say something")
	ErrNoAuthor    = errors.New("a record must name its author")
	ErrNoTarget    = errors.New("a resolution must name the comment it resolves")
	ErrUnknownSide = errors.New("a span is on the old side or the new side")
	ErrEmptySpan   = errors.New("a span covers at least one line, ending at or after it starts")
	ErrMalformed   = errors.New("not a record")
	ErrVersion     = errors.New("a version of the format this build does not speak")
	ErrUnknownKind = errors.New("a kind of record this build does not know")

	// Refusals from the aggregate root.
	ErrNoProposalID    = errors.New("a proposal must be named")
	ErrNoTitle         = errors.New("a proposal must have a title")
	ErrNothingProposed = errors.New("a proposal must move past its base")
	ErrRevisionKnown   = errors.New("that revision is already on the proposal")
	ErrUnknownRevision = errors.New("that revision is not on this proposal")
	ErrUnknownComment  = errors.New("that comment is not on this proposal")
	ErrOpenComments    = errors.New("the head revision still has open comments")
	ErrNotOpen         = errors.New("the proposal is no longer open")
	ErrUnknownState    = errors.New("a state this build does not know")
	ErrNoBranch        = errors.New("a proposal must name the branch it lands on")
	ErrBadBranch       = errors.New("not a branch name git would accept")
	ErrNoCheckName     = errors.New("a check must be named")
	ErrUnknownStatus   = errors.New("a check either passed or failed")
	ErrCheckFailed     = errors.New("a check failed on the head revision")
	ErrCheckMissing    = errors.New("a required check has not run on the head revision")
)
