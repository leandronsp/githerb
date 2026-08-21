package review

// SHA is a full commit object name.
type SHA string

// File is a path inside the repository.
type File string

// ID identifies a record, and is derived from the record's own content, so the
// same annotation written twice is one annotation and the append-only log
// deduplicates itself.
type ID string

// ProposalID names a proposal, and is the last segment of the ref it lives in.
type ProposalID string
