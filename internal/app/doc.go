// Package app holds the use cases: the sequence of steps a command performs,
// and nothing else. Each one is a struct carrying the ports it needs and a
// single exported method, so the dependencies are visible at the call site and
// a test can hand it whatever it likes.
package app
