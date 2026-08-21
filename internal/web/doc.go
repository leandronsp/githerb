// Package web is the review surface: a diff you can annotate, served on
// localhost from the repository you are standing in.
//
// It is server rendered. The browser holds only which lines are selected;
// everything else arrives as HTML over an event stream and is morphed into
// place, so an annotation an agent resolves from the terminal appears here
// without a reload and without losing the selection.
package web
