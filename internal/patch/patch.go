// Package patch reads git's unified diff into something a page can render:
// files, hunks, and lines that know their number on each side.
//
// It is pure. The text arrives from somewhere else and every answer here is a
// function of that text.
package patch

import (
	"errors"
	"strconv"
	"strings"
)

// ErrMalformed is a diff that does not parse.
var ErrMalformed = errors.New("not a unified diff")

// Kind says what happened to a line.
type Kind string

// The four kinds of line a unified diff carries.
const (
	Context Kind = "context"
	Added   Kind = "added"
	Removed Kind = "removed"
	Meta    Kind = "meta"
)

// Line is one row of a hunk. Old and New are the line numbers on each side,
// and are zero where the line does not exist on that side.
type Line struct {
	Kind Kind
	Old  int
	New  int
	Text string
}

// Hunk is one @@ section.
type Hunk struct {
	Header string
	Lines  []Line
}

// File is everything the diff says about one path.
type File struct {
	Path  string
	Hunks []Hunk
}

// Parse reads the output of git diff.
func Parse(diff string) ([]File, error) {
	var (
		files   []File
		current *File
		hunk    *Hunk
		oldLine int
		newLine int
	)

	flushHunk := func() {
		if hunk != nil && current != nil {
			current.Hunks = append(current.Hunks, *hunk)
			hunk = nil
		}
	}

	flushFile := func() {
		flushHunk()

		if current != nil {
			files = append(files, *current)
			current = nil
		}
	}

	for _, raw := range strings.Split(diff, "\n") {
		switch {
		case strings.HasPrefix(raw, "diff --git "):
			flushFile()

			path, err := pathOf(raw)
			if err != nil {
				return nil, err
			}

			current = &File{Path: path, Hunks: nil}
		case strings.HasPrefix(raw, "@@"):
			if current == nil {
				return nil, ErrMalformed
			}

			flushHunk()

			old, next, err := numbers(raw)
			if err != nil {
				return nil, err
			}

			oldLine, newLine = old, next
			hunk = &Hunk{Header: raw, Lines: nil}
		case hunk == nil:
			continue
		case strings.HasPrefix(raw, "+"):
			hunk.Lines = append(hunk.Lines, Line{Kind: Added, Old: 0, New: newLine, Text: raw[1:]})
			newLine++
		case strings.HasPrefix(raw, "-"):
			hunk.Lines = append(hunk.Lines, Line{Kind: Removed, Old: oldLine, New: 0, Text: raw[1:]})
			oldLine++
		case strings.HasPrefix(raw, " "):
			hunk.Lines = append(hunk.Lines, Line{Kind: Context, Old: oldLine, New: newLine, Text: raw[1:]})
			oldLine++
			newLine++
		case strings.HasPrefix(raw, `\`):
			hunk.Lines = append(hunk.Lines, Line{Kind: Meta, Old: 0, New: 0, Text: raw})
		}
	}

	flushFile()

	return files, nil
}

// pathOf takes the new-side path out of a diff --git header.
func pathOf(header string) (string, error) {
	fields := strings.Fields(header)
	if len(fields) < 4 {
		return "", ErrMalformed
	}

	return strings.TrimPrefix(fields[3], "b/"), nil
}

// numbers takes the two starting line numbers out of an @@ header.
func numbers(header string) (int, int, error) {
	body, _, found := strings.Cut(strings.TrimPrefix(header, "@@ "), " @@")
	if !found {
		return 0, 0, ErrMalformed
	}

	sides := strings.Fields(body)
	if len(sides) != 2 {
		return 0, 0, ErrMalformed
	}

	old, err := start(sides[0], "-")
	if err != nil {
		return 0, 0, err
	}

	next, err := start(sides[1], "+")
	if err != nil {
		return 0, 0, err
	}

	return old, next, nil
}

func start(side, sign string) (int, error) {
	first, _, _ := strings.Cut(strings.TrimPrefix(side, sign), ",")

	number, err := strconv.Atoi(first)
	if err != nil {
		return 0, ErrMalformed
	}

	return number, nil
}
