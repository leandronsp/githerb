package review

// Revision is one attempt at a proposal: a commit, and where it sits in the
// sequence. Revision one is the first push, revision two is what the agent
// produced after reading the annotations on revision one.
type Revision struct {
	number int
	sha    SHA
}

// Number is the revision's place in the sequence, starting at one.
func (r Revision) Number() int { return r.number }

// SHA is the commit this revision points at.
func (r Revision) SHA() SHA { return r.sha }
