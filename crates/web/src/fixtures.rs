//! Proposals and diffs to render in tests, built the way the real thing is.
//!
//! Nothing here fakes a repository: a proposal is opened and folded from
//! records, and a patch is parsed from a literal diff, so a test that passes
//! here is a test against the same values the server hands the templates.

use patch::Patch;
use review::{
    Anchor, Author, Branch, Comment, FilePath, Proposal, ProposalId, RecordId, Sha, Side, Span,
    Timestamp,
};

/// A diff of two files, small enough to count the bytes of by hand.
pub const DIFF: &str = "\
diff --git a/README.md b/README.md
index 3b1ba0c..4f1abde 100644
--- a/README.md
+++ b/README.md
@@ -1,4 +1,4 @@
-# githerb
+# githerb (demo)

 Code review and a gate for trunk, in one binary, with no server.

diff --git a/cmd/githerb/extra.go b/cmd/githerb/extra.go
new file mode 100644
index 0000000..0b5c82e
--- /dev/null
+++ b/cmd/githerb/extra.go
@@ -0,0 +1,3 @@
+package main
+
+func extra() int { return 1 }
";

/// A commit, forty hex characters of the one you ask for.
#[must_use]
pub fn sha(mark: char) -> Sha {
    let raw: String = std::iter::repeat_n(mark, 40).collect();
    Sha::parse(&raw).unwrap()
}

/// A moment, so many minutes past midnight on the first of January 2026.
#[must_use]
pub fn at(minutes: i64) -> Timestamp {
    Timestamp::from_unix(1_767_225_600 + minutes * 60)
}

/// Somebody leaving notes.
#[must_use]
pub fn author(name: &str) -> Author {
    Author::parse(name).unwrap()
}

/// An open proposal at revision one, whose head is `bbbb…`.
#[must_use]
pub fn proposal() -> Proposal {
    Proposal::open(
        ProposalId::parse("demo").unwrap(),
        "Demo slice",
        Branch::parse("main").unwrap(),
        sha('a'),
        sha('b'),
        at(0),
    )
    .unwrap()
}

/// The parsed [`DIFF`].
#[must_use]
pub fn patch() -> Patch {
    patch::parse(DIFF).unwrap()
}

/// Where a note points.
#[must_use]
pub fn anchor(file: &str, side: Side, start: u32, end: u32) -> Anchor {
    Anchor::new(
        FilePath::parse(file).unwrap(),
        Span::new(side, start, end).unwrap(),
    )
}

/// Leave a note on a revision, and say what it is called.
pub fn note(
    proposal: &mut Proposal,
    revision: &Sha,
    file: &str,
    side: Side,
    start: u32,
    end: u32,
    body: &str,
) -> RecordId {
    let comment = Comment::new(
        revision.clone(),
        anchor(file, side, start, end),
        body,
        author("leandro"),
        at(46),
    )
    .unwrap();
    let id = comment.id().clone();
    proposal.apply(review::Record::Comment(comment)).unwrap();
    id
}
