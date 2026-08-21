package web

import (
	"encoding/json"
	"fmt"
	"net/http"
	"time"

	"github.com/leandronsp/githerb/internal/app"
	"github.com/leandronsp/githerb/internal/patch"
	"github.com/leandronsp/githerb/internal/review"
)

// selection is what the browser holds: which lines a person is pointing at,
// and what they typed about them.
type selection struct {
	File  string `json:"selFile"`
	Side  string `json:"selSide"`
	Start int    `json:"selStart"`
	End   int    `json:"selEnd"`
	Body  string `json:"body"`
	ID    string `json:"commentID"`
}

func (s Server) index(w http.ResponseWriter, _ *http.Request) {
	proposals, err := s.Proposals.List()
	if err != nil {
		s.fail(w, err)

		return
	}

	s.render(w, "index", map[string]any{"Proposals": proposals})
}

func (s Server) review(w http.ResponseWriter, r *http.Request) {
	page, err := s.page(review.ProposalID(r.PathValue("id")))
	if err != nil {
		s.fail(w, err)

		return
	}

	s.render(w, "review", page)
}

// events keeps the page current. Git has nothing to subscribe to, so this
// watches: it reloads the proposal and pushes the panel whenever what it adds
// up to has changed, which is how an annotation resolved from the terminal
// shows up here.
func (s Server) events(w http.ResponseWriter, r *http.Request) {
	flusher, ok := w.(http.Flusher)
	if !ok {
		s.fail(w, fmt.Errorf("streaming: %w", ErrClosed))

		return
	}

	w.Header().Set("Content-Type", "text/event-stream")
	w.Header().Set("Cache-Control", "no-cache")

	id := review.ProposalID(r.PathValue("id"))
	last := ""

	ticker := time.NewTicker(400 * time.Millisecond)
	defer ticker.Stop()

	for {
		page, err := s.page(id)
		if err != nil {
			return
		}

		if page.Fingerprint != last {
			last = page.Fingerprint

			if err := s.patch(w, "panel", page); err != nil {
				return
			}

			flusher.Flush()
		}

		select {
		case <-r.Context().Done():
			return
		case <-ticker.C:
		}
	}
}

func (s Server) comment(w http.ResponseWriter, r *http.Request) {
	want, err := read(r)
	if err != nil {
		s.fail(w, err)

		return
	}

	use := app.Annotate{Proposals: s.Proposals, Author: s.Author, Now: s.Now}

	if _, err := use.Run(r.PathValue("id"), want.File, want.Side, want.Start, want.End, want.Body); err != nil {
		s.fail(w, err)

		return
	}

	s.clear(w)
}

func (s Server) resolve(w http.ResponseWriter, r *http.Request) {
	want, err := read(r)
	if err != nil {
		s.fail(w, err)

		return
	}

	use := app.Resolve{Proposals: s.Proposals, Author: s.Author, Now: s.Now}

	if err := use.Run(r.PathValue("id"), want.ID); err != nil {
		s.fail(w, err)

		return
	}

	w.WriteHeader(http.StatusNoContent)
}

func (s Server) land(w http.ResponseWriter, r *http.Request) {
	use := app.Land{Proposals: s.Proposals, Author: s.Author, Now: s.Now}

	if _, err := use.Run(r.PathValue("id")); err != nil {
		s.fail(w, err)

		return
	}

	w.WriteHeader(http.StatusNoContent)
}

// clear puts the selection back, so the form empties the moment the comment
// lands rather than waiting for the next stream tick.
func (s Server) clear(w http.ResponseWriter) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(http.StatusOK)

	_ = json.NewEncoder(w).Encode(map[string]any{
		"selFile": "", "selSide": "", "selStart": 0, "selEnd": 0, "body": "",
	})
}

func read(r *http.Request) (selection, error) {
	var want selection

	if err := json.NewDecoder(r.Body).Decode(&want); err != nil {
		return selection{}, fmt.Errorf("reading the selection: %w", err)
	}

	return want, nil
}

func (s Server) page(id review.ProposalID) (Page, error) {
	proposal, err := s.Proposals.Load(id)
	if err != nil {
		return Page{}, err
	}

	raw, err := s.Git.Diff(proposal.Base(), proposal.Head().SHA())
	if err != nil {
		return Page{}, err
	}

	files, err := patch.Parse(raw)
	if err != nil {
		return Page{}, err
	}

	return newPage(proposal, files), nil
}
