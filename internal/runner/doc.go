// Package runner does what the log asks for. It derives the pending work from
// what is written down, claims a job by writing that it started, and runs the
// repository's own agent command in a throwaway worktree.
//
// Nothing here decides what a proposal means. It reads the same records the
// browser reads and acts on them, which is why an agent working from a
// terminal and this loop can never disagree.
package runner
