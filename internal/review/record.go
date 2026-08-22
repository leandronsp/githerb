package review

// Kind names what a record says.
type Kind string

// The kinds of record the log carries.
const (
	KindComment Kind = "comment"
	KindResolve Kind = "resolve"
	KindCheck   Kind = "check"
	// KindChunk is a decision the author is explaining. KindRationale is the
	// author explaining a few lines. Neither blocks landing, because neither is
	// asking for anything.
	KindChunk     Kind = "chunk"
	KindRationale Kind = "rationale"
	// KindWork is an agent saying it picked something up, finished it or gave
	// up on it. Nothing blocks on these; they are how a person watching knows
	// somebody is already on it.
	KindWork Kind = "work"
)

// The absent half of a record. Naming them says the omission is deliberate.
var (
	noComment    Comment
	noResolution Resolution
	noCheck      Check
	noChunk      Chunk
	noWork       Work
)

// Record is one line of the log. Exactly one of its shapes is present, and the
// two-value accessors are the only way to reach them.
type Record struct {
	kind       Kind
	comment    Comment
	resolution Resolution
	check      Check
	chunk      Chunk
	work       Work
}

// CommentRecord wraps a comment as a log record.
func CommentRecord(comment Comment) Record {
	return Record{kind: KindComment, comment: comment, resolution: noResolution, check: noCheck, chunk: noChunk, work: noWork}
}

// ResolutionRecord wraps a resolution as a log record.
func ResolutionRecord(resolution Resolution) Record {
	return Record{kind: KindResolve, comment: noComment, resolution: resolution, check: noCheck, chunk: noChunk, work: noWork}
}

// CheckRecord wraps a check result as a log record.
func CheckRecord(check Check) Record {
	return Record{kind: KindCheck, comment: noComment, resolution: noResolution, check: check, chunk: noChunk, work: noWork}
}

// ChunkRecord wraps a decision as a log record.
func ChunkRecord(chunk Chunk) Record {
	return Record{kind: KindChunk, comment: noComment, resolution: noResolution, check: noCheck, chunk: chunk, work: noWork}
}

// RationaleRecord wraps the author explaining some lines. It is a comment in
// shape and the opposite of one in intent: it answers a question rather than
// asking one, so it never blocks.
func RationaleRecord(comment Comment) Record {
	return Record{kind: KindRationale, comment: comment, resolution: noResolution, check: noCheck, chunk: noChunk, work: noWork}
}

// WorkRecord wraps a line of an agent's work as a log record.
func WorkRecord(work Work) Record {
	return Record{
		kind: KindWork, comment: noComment, resolution: noResolution,
		check: noCheck, chunk: noChunk, work: work,
	}
}

// Work returns the work line, and false when the record is not one.
func (r Record) Work() (Work, bool) {
	return r.work, r.kind == KindWork
}

// Kind says which shape the record carries.
func (r Record) Kind() Kind { return r.kind }

// Comment returns the comment, and false when the record is not one.
func (r Record) Comment() (Comment, bool) {
	return r.comment, r.kind == KindComment || r.kind == KindRationale
}

// Resolution returns the resolution, and false when the record is not one.
func (r Record) Resolution() (Resolution, bool) {
	return r.resolution, r.kind == KindResolve
}

// Check returns the check, and false when the record is not one.
func (r Record) Check() (Check, bool) {
	return r.check, r.kind == KindCheck
}

// Chunk returns the decision, and false when the record is not one.
func (r Record) Chunk() (Chunk, bool) {
	return r.chunk, r.kind == KindChunk
}
