package web

import (
	"testing"

	"github.com/leandronsp/githerb/internal/review"
)

const (
	base = review.SHA("00112233445566778899aabbccddeeff00112233")
	head = review.SHA("9f6c1e2a3b4d5e6f708192a3b4c5d6e7f8091a2b")
)

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
