package web

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strconv"
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

	rows := make([]Row, 0, len(proposals))

	for _, proposal := range proposals {
		added, removed := s.size(proposal)
		rows = append(rows, Row{Proposal: proposal, Added: added, Removed: removed})
	}

	s.render(w, "index", newBoard(rows))
}

// size is what the proposal adds up to. A proposal whose commits have gone
// missing still belongs on the board, so this answers zero rather than failing
// the whole page.
func (s Server) size(proposal review.Proposal) (added, removed int) {
	raw, err := s.Git.Diff(proposal.Base(), proposal.Head().SHA())
	if err != nil {
		return 0, 0
	}

	files, err := patch.Parse(raw)
	if err != nil {
		return 0, 0
	}

	return patch.Count(files)
}

func (s Server) review(w http.ResponseWriter, r *http.Request) {
	since, _ := strconv.Atoi(r.URL.Query().Get("since"))

	page, err := s.page(review.ProposalID(r.PathValue("id")), since)
	if err != nil {
		s.fail(w, err)

		return
	}

	s.render(w, "review", page)
}

// events keeps the page current. Git has nothing to subscribe to, so this
// watches: it reloads the proposal and pushes whatever moved. A new revision
// changes the diff and the decisions too, so that one replaces the page rather
// than the panel, which is what makes this feel like the page reloaded itself.
func (s Server) events(w http.ResponseWriter, r *http.Request) {
	flusher, ok := w.(http.Flusher)
	if !ok {
		s.fail(w, fmt.Errorf("streaming: %w", ErrClosed))

		return
	}

	w.Header().Set("Content-Type", "text/event-stream")
	w.Header().Set("Cache-Control", "no-cache")

	id := review.ProposalID(r.PathValue("id"))
	since, _ := strconv.Atoi(r.URL.Query().Get("since"))

	var mark, head string

	ticker := time.NewTicker(400 * time.Millisecond)
	defer ticker.Stop()

	for {
		page, err := s.page(id, since)
		if err != nil {
			return
		}

		if page.Fingerprint != mark {
			fragment := "panel"
			if string(page.Proposal.Head().SHA()) != head {
				fragment = "page"
			}

			mark = page.Fingerprint
			head = string(page.Proposal.Head().SHA())

			if err := s.patch(w, fragment, page); err != nil {
				return
			}

			// The checks and the buttons sit in the bar, outside the panel, so
			// a record that changes either has to move both.
			if fragment == "panel" {
				if err := s.patch(w, "bar", page); err != nil {
					return
				}
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

// handover hands the whole review to the agent in one piece, which is the
// shape a person actually reviews in: leave notes for an hour, then send them
// all at once.
func (s Server) handover(w http.ResponseWriter, r *http.Request) {
	proposal, err := s.Proposals.Load(review.ProposalID(r.PathValue("id")))
	if err != nil {
		s.fail(w, err)

		return
	}

	w.Header().Set("Content-Type", "text/plain; charset=utf-8")
	w.Header().Set("X-Content-Type-Options", "nosniff")

	// gosec reads any repository content on a response as an injection. This
	// one is plain text the browser is told not to sniff, and the only reader
	// is the clipboard.
	//nolint:gosec // see above
	_, _ = io.WriteString(w, proposal.Handover())
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
	use := app.Land{
		Proposals: s.Proposals, Git: s.Git,
		Required: s.Required, Author: s.Author, Now: s.Now,
	}

	if _, err := use.Run(r.PathValue("id")); err != nil {
		s.fail(w, err)

		return
	}

	w.WriteHeader(http.StatusNoContent)
}

// clear puts the selection back, so the form empties the moment the comment
// lands rather than waiting for the next stream tick.
func (s Server) abandon(w http.ResponseWriter, r *http.Request) {
	use := app.Abandon{Proposals: s.Proposals, Author: s.Author, Now: s.Now}

	if _, err := use.Run(r.PathValue("id")); err != nil {
		s.fail(w, err)

		return
	}

	w.WriteHeader(http.StatusNoContent)
}

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

// page renders either the whole proposal or only what the last revision
// changed, which is what a reviewer coming back to it actually wants to see.
func (s Server) page(id review.ProposalID, since int) (Page, error) {
	proposal, err := s.Proposals.Load(id)
	if err != nil {
		return Page{}, err
	}

	from := proposal.Base()

	for _, revision := range proposal.Revisions() {
		if since > 0 && revision.Number() == since {
			from = revision.SHA()
		}
	}

	raw, err := s.Git.Diff(from, proposal.Head().SHA())
	if err != nil {
		return Page{}, err
	}

	files, err := patch.Parse(raw)
	if err != nil {
		return Page{}, err
	}

	return newPage(proposal, files, s.Required, since), nil
}
