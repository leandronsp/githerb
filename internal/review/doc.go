// Package review holds the vocabulary of a code review and the format it is
// stored in: proposals, the revisions inside them, and the annotations a human
// leaves for an agent to act on.
//
// It is pure. Nothing here reads a file, opens a socket or asks the clock what
// time it is, which is what lets the rules be tested without a repository.
package review
