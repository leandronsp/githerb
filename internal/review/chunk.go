package review

import (
	"fmt"
	"strings"
)

// The caps. They are the whole anti-slop mechanism: a field that must fit
// refuses to hold a paragraph, and the tool says no regardless of which agent
// or which harness wrote it. Instructions are advice; a constructor is a rule.
const (
	maxTitle    = 80
	maxSurface  = 60
	maxSentence = 140
	maxDecision = 200
	maxNoise    = 160
)

// Chunk is one reviewable decision: the thing a person can accept or reject on
// its own. A file may span chunks and a chunk may span files, because the unit
// is the decision and not the path.
type Chunk struct {
	title    string
	surface  string
	before   string
	after    string
	decision string
	rejected string
	file     File
	span     Span
	anchored bool
}

// NewChunk is the only way to build one. Every field is a single line and every
// line has a ceiling.
func NewChunk(title, surface, before, after, decision, rejected string) (Chunk, error) {
	fields := []struct {
		name  string
		value *string
		cap   int
	}{
		{"title", &title, maxTitle},
		{"surface", &surface, maxSurface},
		{"before", &before, maxSentence},
		{"after", &after, maxSentence},
		{"decision", &decision, maxDecision},
		{"rejected", &rejected, maxSentence},
	}

	for _, field := range fields {
		trimmed, err := oneLine(field.name, *field.value, field.cap)
		if err != nil {
			return Chunk{}, err
		}

		*field.value = trimmed
	}

	switch {
	case title == "":
		return Chunk{}, ErrNoTitle
	case before == "" || after == "":
		return Chunk{}, ErrNoBeforeAfter
	case decision == "":
		return Chunk{}, ErrNoDecision
	}

	return Chunk{
		title:    title,
		surface:  surface,
		before:   before,
		after:    after,
		decision: decision,
		rejected: rejected,
		file:     "",
		span:     Span{side: "", start: 0, end: 0},
		anchored: false,
	}, nil
}

// At points the chunk at the lines that carry it, so the page can take the
// reader there instead of asking them to find it.
func (c Chunk) At(file File, span Span) (Chunk, error) {
	if strings.TrimSpace(string(file)) == "" {
		return Chunk{}, ErrNoFile
	}

	c.file = file
	c.span = span
	c.anchored = true

	return c, nil
}

// Title is what the decision is called.
func (c Chunk) Title() string { return c.title }

// Surface is what a person touches, or "internal".
func (c Chunk) Surface() string { return c.surface }

// Before is how it worked, in one line, in product language.
func (c Chunk) Before() string { return c.before }

// After is how it works now, in one line.
func (c Chunk) After() string { return c.after }

// Decision is the call that was made.
func (c Chunk) Decision() string { return c.decision }

// Rejected is the alternative that was not taken, when there was one.
func (c Chunk) Rejected() string { return c.rejected }

// File is the path the chunk points at.
func (c Chunk) File() File { return c.file }

// Span is the range the chunk points at.
func (c Chunk) Span() Span { return c.span }

// Anchored reports whether the chunk points anywhere.
func (c Chunk) Anchored() bool { return c.anchored }

// oneLine is where prolixity dies.
func oneLine(name, value string, cap int) (string, error) {
	trimmed := strings.TrimSpace(value)

	if strings.ContainsAny(trimmed, "\n\r") {
		return "", fmt.Errorf("%s: %w", name, ErrNotOneLine)
	}

	if len([]rune(trimmed)) > cap {
		return "", fmt.Errorf("%s is %d characters, the ceiling is %d: %w",
			name, len([]rune(trimmed)), cap, ErrTooLong)
	}

	return trimmed, nil
}
