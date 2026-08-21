package app

import (
	"encoding/json"
	"fmt"
	"io"

	"github.com/leandronsp/githerb/internal/review"
)

// Describe attaches the reasoning to a proposal: the decisions it carries and
// the lines the author wants to explain before anyone asks.
//
// It takes JSON rather than flags because the writer is usually an agent, and
// the domain caps every field, so a description cannot ramble no matter which
// agent or which harness produced it.
type Describe struct {
	Proposals review.Proposals
	Author    string
	Now       Clock
}

// Description is the shape an author submits.
type Description struct {
	Chunks    []ChunkInput     `json:"chunks"`
	Rationale []RationaleInput `json:"rationale"`
}

// ChunkInput is one reviewable decision. Every field is one line and the
// domain refuses anything longer than its ceiling.
type ChunkInput struct {
	Title    string `json:"title"`
	Surface  string `json:"surface"`
	Before   string `json:"before"`
	After    string `json:"after"`
	Decision string `json:"decision"`
	Rejected string `json:"rejected,omitempty"`
	File     string `json:"file,omitempty"`
	Side     string `json:"side,omitempty"`
	Start    int    `json:"start,omitempty"`
	End      int    `json:"end,omitempty"`
}

// RationaleInput is the author explaining a few lines.
type RationaleInput struct {
	File  string `json:"file"`
	Side  string `json:"side,omitempty"`
	Start int    `json:"start"`
	End   int    `json:"end,omitempty"`
	Body  string `json:"body"`
}

// Run reads the description and writes it against the head revision.
func (d Describe) Run(id string, from io.Reader) (int, error) {
	proposal, err := d.Proposals.Load(review.ProposalID(id))
	if err != nil {
		return 0, err
	}

	var description Description

	if err := json.NewDecoder(from).Decode(&description); err != nil {
		return 0, fmt.Errorf("reading the description: %w", err)
	}

	head := proposal.Head().SHA()
	written := 0

	for _, input := range description.Chunks {
		chunk, err := d.chunk(input)
		if err != nil {
			return written, err
		}

		if err := d.Proposals.Annotate(head, review.ChunkRecord(chunk)); err != nil {
			return written, err
		}

		written++
	}

	for _, input := range description.Rationale {
		comment, err := d.rationale(input, head)
		if err != nil {
			return written, err
		}

		if err := d.Proposals.Annotate(head, review.RationaleRecord(comment)); err != nil {
			return written, err
		}

		written++
	}

	return written, nil
}

func (d Describe) chunk(input ChunkInput) (review.Chunk, error) {
	chunk, err := review.NewChunk(
		input.Title, input.Surface, input.Before, input.After, input.Decision, input.Rejected,
	)
	if err != nil {
		return review.Chunk{}, err
	}

	if input.File == "" {
		return chunk, nil
	}

	span, err := spanOf(input.Side, input.Start, input.End)
	if err != nil {
		return review.Chunk{}, err
	}

	anchored, err := chunk.At(review.File(input.File), span)
	if err != nil {
		return review.Chunk{}, err
	}

	return anchored, nil
}

func (d Describe) rationale(input RationaleInput, head review.SHA) (review.Comment, error) {
	span, err := spanOf(input.Side, input.Start, input.End)
	if err != nil {
		return review.Comment{}, err
	}

	comment, err := review.NewComment(head, review.File(input.File), span, input.Body, d.Author, d.Now())
	if err != nil {
		return review.Comment{}, err
	}

	return comment, nil
}

// spanOf fills in what an author left out: the new side, and a single line.
func spanOf(side string, start, end int) (review.Span, error) {
	if side == "" {
		side = string(review.SideNew)
	}

	if end == 0 {
		end = start
	}

	parsed, err := review.ParseSide(side)
	if err != nil {
		return review.Span{}, err
	}

	span, err := review.NewSpan(parsed, start, end)
	if err != nil {
		return review.Span{}, err
	}

	return span, nil
}
