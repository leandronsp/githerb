package review

// Kind names what a record says.
type Kind string

// The kinds of record the log carries.
const (
	KindComment Kind = "comment"
	KindResolve Kind = "resolve"
	KindCheck   Kind = "check"
)

// The absent half of a record. Naming them says the omission is deliberate.
var (
	noComment    Comment
	noResolution Resolution
	noCheck      Check
)

// Record is one line of the log. Exactly one of its shapes is present, and the
// two-value accessors are the only way to reach them.
type Record struct {
	kind       Kind
	comment    Comment
	resolution Resolution
	check      Check
}

// CommentRecord wraps a comment as a log record.
func CommentRecord(comment Comment) Record {
	return Record{kind: KindComment, comment: comment, resolution: noResolution, check: noCheck}
}

// ResolutionRecord wraps a resolution as a log record.
func ResolutionRecord(resolution Resolution) Record {
	return Record{kind: KindResolve, comment: noComment, resolution: resolution, check: noCheck}
}

// CheckRecord wraps a check result as a log record.
func CheckRecord(check Check) Record {
	return Record{kind: KindCheck, comment: noComment, resolution: noResolution, check: check}
}

// Kind says which shape the record carries.
func (r Record) Kind() Kind { return r.kind }

// Comment returns the comment, and false when the record is not one.
func (r Record) Comment() (Comment, bool) {
	return r.comment, r.kind == KindComment
}

// Resolution returns the resolution, and false when the record is not one.
func (r Record) Resolution() (Resolution, bool) {
	return r.resolution, r.kind == KindResolve
}

// Check returns the check, and false when the record is not one.
func (r Record) Check() (Check, bool) {
	return r.check, r.kind == KindCheck
}
