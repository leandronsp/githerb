// Package gitstore keeps proposals in the repository they are about.
//
// A proposal is a ref per revision under refs/githerb/proposals, its life is a
// note on the first revision's commit, and the annotations are a note on the
// commit they apply to. Nothing else is written, so every proposal and every
// comment travels over any git transport that already exists, and a team needs
// no server for a colleague to fetch them.
//
// It shells out to the git binary rather than reimplementing it, because git
// is the one program guaranteed to agree with git.
package gitstore
