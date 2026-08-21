package review_test

import (
	"strings"
	"testing"

	"github.com/leandronsp/githerb/internal/review"
)

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
