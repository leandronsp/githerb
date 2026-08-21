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
)
