package review

import "fmt"

// Side names which column of a diff a span belongs to.
type Side string

// The two sides of a diff. A comment on a deleted line is on the old side.
const (
	SideOld Side = "old"
	SideNew Side = "new"
)

// ParseSide turns untrusted input into a Side, and is the only door into one.
func ParseSide(raw string) (Side, error) {
	switch Side(raw) {
	case SideOld:
		return SideOld, nil
	case SideNew:
		return SideNew, nil
	default:
		return "", fmt.Errorf("%q: %w", raw, ErrUnknownSide)
	}
}

// Span is a range of lines on one side of a diff, inclusive at both ends. A
// single line is a span whose start and end are equal.
type Span struct {
	side  Side
	start int
	end   int
}

// NewSpan is the only way to build a Span, so an invalid one cannot exist.
func NewSpan(side Side, start, end int) (Span, error) {
	if _, err := ParseSide(string(side)); err != nil {
		return Span{side: "", start: 0, end: 0}, err
	}

	if start < 1 || end < start {
		return Span{side: "", start: 0, end: 0},
			fmt.Errorf("lines %d to %d: %w", start, end, ErrEmptySpan)
	}

	return Span{side: side, start: start, end: end}, nil
}

// Side reports which column of the diff the span is on.
func (s Span) Side() Side { return s.side }

// Start is the first line the span covers.
func (s Span) Start() int { return s.start }

// End is the last line the span covers.
func (s Span) End() int { return s.end }

// Lines is how many lines the span covers.
func (s Span) Lines() int { return s.end - s.start + 1 }
