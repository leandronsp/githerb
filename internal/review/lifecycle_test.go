package review_test

import (
	"errors"
	"testing"

	"github.com/leandronsp/githerb/internal/review"
)

func TestEventsRoundTrip(t *testing.T) {
	t.Parallel()

	opened, err := review.Opened("gate", "Land the gate", "main", base, "leandro", at(t))
	if err != nil {
		t.Fatalf("opened: %v", err)
	}

	landed, err := review.Landed("gate", "leandro", at(t))
	if err != nil {
		t.Fatalf("landed: %v", err)
	}

	for _, want := range []review.Event{opened, landed} {
		t.Run(string(want.Kind()), func(t *testing.T) {
			t.Parallel()

			line, err := want.MarshalLine()
			if err != nil {
				t.Fatalf("marshal: %v", err)
			}

			got, err := review.ParseEvent(line)
			if err != nil {
				t.Fatalf("parse: %v", err)
			}

			if got != want {
				t.Fatalf("round trip changed the event\n got %#v\nwant %#v", got, want)
			}
		})
	}
}

func TestTheOpenedLineIsStable(t *testing.T) {
	t.Parallel()

	const golden = `{"v":1,"kind":"opened","id":"gate","title":"Land the gate","target":"main","base":"00112233445566778899aabbccddeeff00112233","author":"leandro","at":"2026-08-21T18:04:05Z"}`

	opened, err := review.Opened("gate", "Land the gate", "main", base, "leandro", at(t))
	if err != nil {
		t.Fatalf("opened: %v", err)
	}

	line, err := opened.MarshalLine()
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}

	if string(line) != golden {
		t.Fatalf("the proposal log format changed\n got %s\nwant %s", line, golden)
	}
}

func TestRefusedEvents(t *testing.T) {
	t.Parallel()

	t.Run("opened without a base", func(t *testing.T) {
		t.Parallel()

		_, err := review.Opened("gate", "t", "main", "nope", "leandro", at(t))
		if !errors.Is(err, review.ErrNoRevision) {
			t.Fatalf("got %v", err)
		}
	})

	t.Run("landed without an author", func(t *testing.T) {
		t.Parallel()

		if _, err := review.Landed("gate", "  ", at(t)); !errors.Is(err, review.ErrNoAuthor) {
			t.Fatalf("got %v", err)
		}
	})

	t.Run("a line we do not speak", func(t *testing.T) {
		t.Parallel()

		for _, raw := range []string{"", "{}", `{"v":9,"kind":"opened"}`, `{"v":1,"kind":"shipped","author":"l","at":"2026-08-21T18:04:05Z"}`} {
			if _, err := review.ParseEvent([]byte(raw)); err == nil {
				t.Fatalf("accepted %q", raw)
			}
		}
	})
}
