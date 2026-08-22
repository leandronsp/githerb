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
	// KindDispatch is a person handing the open notes to an agent, which is the
	// only thing in the log that asks for work rather than reporting it.
	KindDispatch Kind = "dispatch"
	// KindReply is an answer to a note, from a person or from an agent. It is
	// what turns a note into a conversation instead of a ticket.
	KindReply Kind = "reply"
)

// The absent half of a record. Naming them says the omission is deliberate.
var (
	noComment    Comment
	noResolution Resolution
	noCheck      Check
	noChunk      Chunk
	noWork       Work
	noDispatch   Dispatch
	noReply      Reply
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
	dispatch   Dispatch
	reply      Reply
}

// CommentRecord wraps a comment as a log record.
func CommentRecord(comment Comment) Record {
	return Record{kind: KindComment, comment: comment, resolution: noResolution, check: noCheck, chunk: noChunk, work: noWork, dispatch: noDispatch, reply: noReply}
}

// ResolutionRecord wraps a resolution as a log record.
func ResolutionRecord(resolution Resolution) Record {
	return Record{kind: KindResolve, comment: noComment, resolution: resolution, check: noCheck, chunk: noChunk, work: noWork, dispatch: noDispatch, reply: noReply}
}

// CheckRecord wraps a check result as a log record.
func CheckRecord(check Check) Record {
	return Record{kind: KindCheck, comment: noComment, resolution: noResolution, check: check, chunk: noChunk, work: noWork, dispatch: noDispatch, reply: noReply}
}

// ChunkRecord wraps a decision as a log record.
func ChunkRecord(chunk Chunk) Record {
	return Record{kind: KindChunk, comment: noComment, resolution: noResolution, check: noCheck, chunk: chunk, work: noWork, dispatch: noDispatch, reply: noReply}
}

// RationaleRecord wraps the author explaining some lines. It is a comment in
// shape and the opposite of one in intent: it answers a question rather than
// asking one, so it never blocks.
func RationaleRecord(comment Comment) Record {
	return Record{kind: KindRationale, comment: comment, resolution: noResolution, check: noCheck, chunk: noChunk, work: noWork, dispatch: noDispatch, reply: noReply}
}

// WorkRecord wraps a line of an agent's work as a log record.
func WorkRecord(work Work) Record {
	return Record{
		kind: KindWork, comment: noComment, resolution: noResolution,
		check: noCheck, chunk: noChunk, work: work, dispatch: noDispatch, reply: noReply,
	}
}

// Work returns the work line, and false when the record is not one.
func (r Record) Work() (Work, bool) {
	return r.work, r.kind == KindWork
}

// DispatchRecord wraps a request for an agent as a log record.
func DispatchRecord(dispatch Dispatch) Record {
	return Record{
		kind: KindDispatch, comment: noComment, resolution: noResolution,
		check: noCheck, chunk: noChunk, work: noWork, dispatch: dispatch, reply: noReply,
	}
}

// Dispatch returns the request, and false when the record is not one.
func (r Record) Dispatch() (Dispatch, bool) {
	return r.dispatch, r.kind == KindDispatch
}

// ReplyRecord wraps an answer to a note as a log record.
func ReplyRecord(reply Reply) Record {
	return Record{
		kind: KindReply, comment: noComment, resolution: noResolution,
		check: noCheck, chunk: noChunk, work: noWork, dispatch: noDispatch, reply: reply,
	}
}

// Reply returns the answer, and false when the record is not one.
func (r Record) Reply() (Reply, bool) {
	return r.reply, r.kind == KindReply
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
