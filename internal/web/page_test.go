package web

import (
	"testing"

	"github.com/leandronsp/githerb/internal/review"
)

const (
	base = review.SHA("00112233445566778899aabbccddeeff00112233")
	head = review.SHA("9f6c1e2a3b4d5e6f708192a3b4c5d6e7f8091a2b")
)

func decided(t *testing.T, title, surface, file string, line int) review.Chunk {
	t.Helper()

	chunk, err := review.NewChunk(title, surface, "before", "after", "the call", "")
	if err != nil {
		t.Fatalf("chunk: %v", err)
	}

	span, err := review.NewSpan(review.SideNew, line, line)
	if err != nil {
		t.Fatalf("span: %v", err)
	}

	anchored, err := chunk.At(review.File(file), span)
	if err != nil {
		t.Fatalf("anchor: %v", err)
	}

	return anchored
}

func pageWith(t *testing.T, chunks ...review.Chunk) Page {
	t.Helper()

	proposal, err := review.NewProposal("p", "A proposal", "main", base, head)
	if err != nil {
		t.Fatalf("proposal: %v", err)
	}

	for _, chunk := range chunks {
		proposal, err = proposal.WithRecord(review.ChunkRecord(chunk))
		if err != nil {
			t.Fatalf("record: %v", err)
		}
	}

	return newPage(proposal, nil, nil, 0)
}

func TestADecisionIsFoundByFileAndByLine(t *testing.T) {
	t.Parallel()

	page := pageWith(t,
		decided(t, "One", "internal/web", "internal/web/page.go", 12),
		decided(t, "Two", "the stream", "internal/web/handlers.go", 81),
	)

	if got := len(page.Decisions()); got != 2 {
		t.Fatalf("the page numbered %d decisions, want 2", got)
	}

	inFile := page.DecidesIn("internal/web/handlers.go")
	if len(inFile) != 1 || inFile[0].Chunk.Title() != "Two" {
		t.Fatalf("the file carries %+v", inFile)
	}

	atLine := page.DecidesAt("internal/web/handlers.go", review.SideNew, 81)
	if len(atLine) != 1 || atLine[0].N != 2 {
		t.Fatalf("the line carries %+v, want decision 2", atLine)
	}

	if other := page.DecidesAt("internal/web/handlers.go", review.SideNew, 80); len(other) != 0 {
		t.Fatalf("a line nothing points at carries %+v", other)
	}
}

func TestASurfaceThatOnlyRepeatsTheFileIsDropped(t *testing.T) {
	t.Parallel()

	page := pageWith(t,
		decided(t, "One", "internal/web", "internal/web/page.go", 12),
		decided(t, "Two", "the stream", "internal/web/handlers.go", 81),
	)

	if got := page.Decisions()[0].Surface(); got != "" {
		t.Fatalf("the surface repeated the file and survived as %q", got)
	}

	if got := page.Decisions()[1].Surface(); got != "the stream" {
		t.Fatalf("the surface is %q, want the stream", got)
	}
}

func TestABoardGroupsByState(t *testing.T) {
	t.Parallel()

	open, err := review.NewProposal("open", "Open one", "main", base, head)
	if err != nil {
		t.Fatalf("proposal: %v", err)
	}

	landed, err := open.Landed()
	if err != nil {
		t.Fatalf("land: %v", err)
	}

	board := newBoard([]Row{
		{Proposal: open, Added: 3, Removed: 1},
		{Proposal: landed, Added: 3, Removed: 1},
	})

	if len(board.Open) != 1 || len(board.Landed) != 1 || len(board.Abandoned) != 0 {
		t.Fatalf("the board grouped %d open, %d landed, %d abandoned",
			len(board.Open), len(board.Landed), len(board.Abandoned))
	}

	if row := board.Open[0]; row.ID() != "open" || row.Revision() != 1 || row.Added != 3 {
		t.Fatalf("the row reads %+v", row)
	}
}
