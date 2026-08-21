package review_test

import (
	"errors"
	"strings"
	"testing"
	"time"

	"github.com/leandronsp/githerb/internal/review"
)

const (
	rev  = review.SHA("9f6c1e2a3b4d5e6f708192a3b4c5d6e7f8091a2b")
	file = review.File("internal/app/land.go")
)

func at(t *testing.T) time.Time {
	t.Helper()

	moment, err := time.Parse(time.RFC3339, "2026-08-21T18:04:05Z")
	if err != nil {
		t.Fatalf("fixture clock: %v", err)
	}

	return moment
}

func comment(t *testing.T, body string) review.Comment {
	t.Helper()

	span, err := review.NewSpan(review.SideNew, 42, 47)
	if err != nil {
		t.Fatalf("span: %v", err)
	}

	made, err := review.NewComment(rev, file, span, body, "leandro", at(t))
	if err != nil {
		t.Fatalf("comment: %v", err)
	}

	return made
}

func TestCommentRoundTrips(t *testing.T) {
	t.Parallel()

	want := comment(t, "this leaks the handle when init fails")

	line, err := want.MarshalLine()
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}

	if strings.Contains(string(line), "\n") {
		t.Fatalf("a record must be one line, got %q", line)
	}

	entry, err := review.ParseLine(line)
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	got, ok := entry.Comment()
	if !ok {
		t.Fatalf("parsed a %s, want a comment", entry.Kind())
	}

	if got != want {
		t.Fatalf("round trip changed the record\n got %#v\nwant %#v", got, want)
	}
}

func TestCommentIDIsDerivedFromContent(t *testing.T) {
	t.Parallel()

	first := comment(t, "same words")
	second := comment(t, "same words")
	other := comment(t, "different words")

	if first.ID() != second.ID() {
		t.Fatalf("the same content produced two ids, %q and %q", first.ID(), second.ID())
	}

	if first.ID() == other.ID() {
		t.Fatalf("different content produced the same id, %q", first.ID())
	}
}

func TestTheLineIsStable(t *testing.T) {
	t.Parallel()

	const golden = `{"v":1,"kind":"comment","id":"9b052da286a4","rev":"9f6c1e2a3b4d5e6f708192a3b4c5d6e7f8091a2b","file":"internal/app/land.go","side":"new","start":42,"end":47,"body":"this leaks the handle when init fails","author":"leandro","at":"2026-08-21T18:04:05Z"}`

	line, err := comment(t, "this leaks the handle when init fails").MarshalLine()
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}

	if string(line) != golden {
		t.Fatalf("the wire format changed, which breaks every agent reading it\n got %s\nwant %s", line, golden)
	}
}

func TestResolutionRoundTrips(t *testing.T) {
	t.Parallel()

	target := comment(t, "rename this")

	want, err := review.NewResolution(target.ID(), "claude", at(t))
	if err != nil {
		t.Fatalf("resolution: %v", err)
	}

	line, err := want.MarshalLine()
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}

	entry, err := review.ParseLine(line)
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	got, ok := entry.Resolution()
	if !ok {
		t.Fatalf("parsed a %s, want a resolution", entry.Kind())
	}

	if got != want {
		t.Fatalf("round trip changed the record\n got %#v\nwant %#v", got, want)
	}
}

func TestRefusedComments(t *testing.T) {
	t.Parallel()

	span, err := review.NewSpan(review.SideNew, 1, 1)
	if err != nil {
		t.Fatalf("span: %v", err)
	}

	cases := []struct {
		name   string
		rev    review.SHA
		file   review.File
		body   string
		author string
		want   error
	}{
		{"no revision", "", file, "x", "leandro", review.ErrNoRevision},
		{"revision is not a sha", "nope", file, "x", "leandro", review.ErrNoRevision},
		{"no file", rev, "", "x", "leandro", review.ErrNoFile},
		{"no body", rev, file, "   ", "leandro", review.ErrNoBody},
		{"no author", rev, file, "x", "", review.ErrNoAuthor},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()

			_, err := review.NewComment(tc.rev, tc.file, span, tc.body, tc.author, at(t))
			if !errors.Is(err, tc.want) {
				t.Fatalf("got %v, want %v", err, tc.want)
			}
		})
	}
}

func TestRefusedSpans(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name  string
		side  review.Side
		start int
		end   int
		want  error
	}{
		{"unknown side", review.Side("sideways"), 1, 1, review.ErrUnknownSide},
		{"line zero", review.SideNew, 0, 1, review.ErrEmptySpan},
		{"negative line", review.SideOld, -3, -1, review.ErrEmptySpan},
		{"end before start", review.SideNew, 9, 4, review.ErrEmptySpan},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()

			_, err := review.NewSpan(tc.side, tc.start, tc.end)
			if !errors.Is(err, tc.want) {
				t.Fatalf("got %v, want %v", err, tc.want)
			}
		})
	}
}

func TestRefusedLines(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name string
		line string
	}{
		{"not json", "hello there"},
		{"empty", ""},
		{"no kind", `{"v":1,"id":"abc"}`},
		{"unknown kind", `{"v":1,"kind":"applause","id":"abc"}`},
		{"a version we do not speak", `{"v":99,"kind":"comment","id":"abc"}`},
		{"a comment missing its span", `{"v":1,"kind":"comment","id":"abc","rev":"9f6c1e2a3b4d5e6f708192a3b4c5d6e7f8091a2b","file":"a.go","side":"new","body":"x","author":"l","at":"2026-08-21T18:04:05Z"}`},
		{"a resolution pointing nowhere", `{"v":1,"kind":"resolve","id":"abc","author":"l","at":"2026-08-21T18:04:05Z"}`},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()

			if _, err := review.ParseLine([]byte(tc.line)); err == nil {
				t.Fatalf("accepted %q", tc.line)
			}
		})
	}
}
