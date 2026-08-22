//! Everything one render of the review page needs, computed once.
//!
//! The page is built from a proposal and a parsed diff, and after that the
//! renderer only walks what is already here. Nothing in `render` asks a
//! question that costs a scan, because every such question was answered on
//! the way in: which threads sit under a row, which rows a note covers, where
//! a decision starts, what the checks say.
//!
//! The old surface did the opposite and called four linear searches per diff
//! line per render, which is why a nine thousand line diff froze the tab.

pub mod board;
pub mod fingerprint;
pub mod rail;
pub mod rows;
pub mod status;
pub mod threads;

use std::collections::HashMap;

use review::{CheckName, Proposal, State, Timestamp};

pub use board::Board;
pub use fingerprint::fingerprint;
pub use rail::{Decision, Entry};
pub use rows::{FileView, HunkView, Row, RowKind};
pub use status::{Agent, AgentState, CheckRow, CheckState, Origin};
pub use threads::{Thread, Turn};

/// Above this many rows in the whole diff, the big files are folded away and
/// fetched when the reader asks for them.
const INLINE_CEILING: usize = 3000;

/// A file with more rows than this is the one that gets folded.
const LARGE_FILE: usize = 400;

/// One review page, ready to render.
#[derive(Debug, Clone)]
pub struct Page {
    id: String,
    title: String,
    target: String,
    state: State,
    head: u32,
    since: u32,
    added: usize,
    removed: usize,
    files: Vec<FileView>,
    threads: Vec<Thread>,
    decisions: Vec<Decision>,
    checks: Vec<CheckRow>,
    agent: Agent,
    blocked: Option<String>,
    origins: Vec<Origin>,
    timeline: Vec<Entry>,
    open_notes: usize,
    fingerprint: String,
}

impl Page {
    /// Build the page.
    ///
    /// `since` is the revision the diff is measured from, `None` for the base.
    /// `required` is what the repository declares its checks to be, which is
    /// what decides whether a missing check blocks.
    #[must_use]
    pub fn build(
        proposal: &Proposal,
        patch: &patch::Patch,
        required: &[CheckName],
        since: Option<u32>,
    ) -> Self {
        let index_of: HashMap<&str, usize> = patch
            .files()
            .iter()
            .enumerate()
            .map(|(index, file)| (file.path(), index))
            .collect();

        let mut threads = threads::threads(proposal);
        let anchors = threads::Anchors::build(proposal, &threads, &index_of);

        let mut decision_rows = HashMap::new();
        let mut files = Vec::with_capacity(patch.files().len());
        for (index, file) in patch.files().iter().enumerate() {
            files.push(rows::build(
                index,
                file,
                &anchors,
                &mut threads,
                &mut decision_rows,
            ));
        }
        collapse(&mut files);

        Self {
            id: proposal.id().as_str().to_owned(),
            title: proposal.title().to_owned(),
            target: proposal.target().as_str().to_owned(),
            state: proposal.state(),
            head: proposal.head().number(),
            since: since.unwrap_or(0),
            added: patch.added(),
            removed: patch.removed(),
            decisions: rail::decisions(proposal, &decision_rows),
            checks: status::checks(proposal, required),
            agent: status::agent(proposal),
            blocked: proposal.landable(required).err().map(|why| why.to_string()),
            origins: status::origins(proposal, since),
            timeline: rail::timeline(proposal),
            open_notes: proposal.open_comments().len(),
            fingerprint: fingerprint(proposal),
            files,
            threads,
        }
    }

    /// The proposal's id, which every route and every form carries.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// What the proposal is called.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The branch it lands on.
    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Open, landed or abandoned.
    #[must_use]
    pub fn state(&self) -> State {
        self.state
    }

    /// Which revision the diff ends at.
    #[must_use]
    pub fn head(&self) -> u32 {
        self.head
    }

    /// Which revision the diff is measured from, zero for the base.
    #[must_use]
    pub fn since(&self) -> u32 {
        self.since
    }

    /// Lines added across the whole diff.
    #[must_use]
    pub fn added(&self) -> usize {
        self.added
    }

    /// Lines removed across the whole diff.
    #[must_use]
    pub fn removed(&self) -> usize {
        self.removed
    }

    /// The files, in the order git listed them.
    #[must_use]
    pub fn files(&self) -> &[FileView] {
        &self.files
    }

    /// One file by its index, which is what a lazy load asks for.
    #[must_use]
    pub fn file(&self, index: usize) -> Option<&FileView> {
        self.files.get(index)
    }

    /// Every thread, resolved or not, anchored or not.
    #[must_use]
    pub fn threads(&self) -> &[Thread] {
        &self.threads
    }

    /// One thread by position, which is what a row carries.
    #[must_use]
    pub fn thread(&self, position: usize) -> Option<&Thread> {
        self.threads.get(position)
    }

    /// The threads that render inside the diff: unresolved, and on a line the
    /// diff still shows. This is what the event stream pushes.
    #[must_use]
    pub fn inline_threads(&self) -> Vec<&Thread> {
        self.threads
            .iter()
            .filter(|thread| !thread.resolved() && thread.anchored())
            .collect()
    }

    /// Unresolved notes written against the head.
    #[must_use]
    pub fn notes_open(&self) -> Vec<&Thread> {
        self.threads
            .iter()
            .filter(|thread| !thread.resolved() && thread.stale().is_none())
            .collect()
    }

    /// Unresolved notes left on a revision that has been superseded.
    #[must_use]
    pub fn notes_earlier(&self) -> Vec<&Thread> {
        self.threads
            .iter()
            .filter(|thread| !thread.resolved() && thread.stale().is_some())
            .collect()
    }

    /// Notes somebody has answered.
    #[must_use]
    pub fn notes_resolved(&self) -> Vec<&Thread> {
        self.threads
            .iter()
            .filter(|thread| thread.resolved())
            .collect()
    }

    /// What the author explained.
    #[must_use]
    pub fn decisions(&self) -> &[Decision] {
        &self.decisions
    }

    /// The checks on the head revision, and the required ones that are absent.
    #[must_use]
    pub fn checks(&self) -> &[CheckRow] {
        &self.checks
    }

    /// Who is working on it.
    #[must_use]
    pub fn agent(&self) -> &Agent {
        &self.agent
    }

    /// Why it cannot land, and nothing when it can.
    #[must_use]
    pub fn blocked(&self) -> Option<&str> {
        self.blocked.as_deref()
    }

    /// Whether the gate is open.
    #[must_use]
    pub fn landable(&self) -> bool {
        self.blocked.is_none()
    }

    /// The ends the diff can be measured from.
    #[must_use]
    pub fn origins(&self) -> &[Origin] {
        &self.origins
    }

    /// Whether there is more than one revision, which is when the strip is
    /// worth showing at all.
    #[must_use]
    pub fn revised(&self) -> bool {
        self.head > 1
    }

    /// The last few things an agent did, newest first.
    #[must_use]
    pub fn timeline(&self) -> &[Entry] {
        &self.timeline
    }

    /// How many notes on the head nobody has resolved. This is what blocks,
    /// and what the dispatch button counts.
    #[must_use]
    pub fn open_notes(&self) -> usize {
        self.open_notes
    }

    /// The hash of everything drawn here, compared on every stream connect.
    #[must_use]
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
}

/// Fold the big files away when the whole diff is too large to hand over at
/// once. A small diff is always inline, however lopsided its files are.
fn collapse(files: &mut [FileView]) {
    let total: usize = files.iter().map(FileView::lines).sum();
    if total <= INLINE_CEILING {
        return;
    }
    for file in files.iter_mut().filter(|file| file.lines() > LARGE_FILE) {
        file.collapse();
    }
}

/// `HH:MM` out of a timestamp, which is what a line of a log has room for.
/// The written shape is fixed at `YYYY-MM-DDTHH:MM:SSZ`.
pub(crate) fn clock(at: Timestamp) -> String {
    at.to_string().get(11..16).unwrap_or_default().to_owned()
}

#[cfg(test)]
mod tests {
    use std::fmt::Write;

    use super::*;
    use crate::fixtures::{DIFF, at, author, note, patch, proposal, sha};
    use review::{Chunk, Comment, Rationale, Record, Resolution, Side};

    /// The page as the server builds it, with no checks required.
    fn page(proposal: &review::Proposal, patch: &patch::Patch) -> Page {
        Page::build(proposal, patch, &[], None)
    }

    /// The row at that position of that file.
    fn row(page: &Page, file: usize, position: usize) -> &Row {
        page.file(file)
            .unwrap()
            .hunks()
            .iter()
            .flat_map(HunkView::rows)
            .nth(position)
            .unwrap()
    }

    // --- anchoring ---

    #[test]
    fn a_thread_anchors_to_the_last_line_of_its_span() {
        let mut proposal = proposal();
        let head = proposal.head().sha().clone();
        note(
            &mut proposal,
            &head,
            "README.md",
            Side::New,
            2,
            3,
            "these two want a name",
        );
        let page = page(&proposal, &patch());
        assert_eq!(page.threads()[0].row(), "L-0-3");
        assert_eq!(row(&page, 0, 3).threads(), &[0]);
        assert_eq!(row(&page, 0, 2).threads(), &[] as &[usize]);
    }

    #[test]
    fn noted_rows_cover_the_whole_span() {
        let mut proposal = proposal();
        let head = proposal.head().sha().clone();
        note(&mut proposal, &head, "README.md", Side::New, 2, 3, "both");
        let page = page(&proposal, &patch());
        let noted: Vec<bool> = (0..5).map(|row| self::row(&page, 0, row).noted()).collect();
        assert_eq!(noted, vec![false, false, true, true, false]);
    }

    #[test]
    fn a_resolved_note_marks_no_rows_and_leaves_the_diff() {
        let mut proposal = proposal();
        let head = proposal.head().sha().clone();
        let id = note(&mut proposal, &head, "README.md", Side::New, 2, 3, "both");
        proposal
            .apply(Record::Resolve(Resolution::new(
                id,
                author("leandro"),
                at(50),
            )))
            .unwrap();
        let page = page(&proposal, &patch());
        assert!(!row(&page, 0, 3).noted(), "a resolved note marks nothing");
        assert_eq!(page.inline_threads().len(), 0);
        assert_eq!(page.notes_resolved().len(), 1);
    }

    #[test]
    fn a_note_left_on_an_older_revision_says_which_one() {
        let mut proposal = proposal();
        let first = proposal.head().sha().clone();
        note(
            &mut proposal,
            &first,
            "README.md",
            Side::New,
            1,
            1,
            "drop it",
        );
        proposal.add_revision(sha('c')).unwrap();
        let page = page(&proposal, &patch());
        assert_eq!(page.threads()[0].stale(), Some("on r1"));
        assert_eq!(page.notes_earlier().len(), 1);
        assert_eq!(page.notes_open().len(), 0);
    }

    #[test]
    fn a_note_on_a_file_the_diff_does_not_show_is_listed_and_not_anchored() {
        let mut proposal = proposal();
        let head = proposal.head().sha().clone();
        note(&mut proposal, &head, "gone.txt", Side::New, 1, 1, "where?");
        let page = page(&proposal, &patch());
        assert!(!page.threads()[0].anchored(), "no row to sit under");
        assert_eq!(page.inline_threads().len(), 0);
        assert_eq!(page.notes_open().len(), 1);
    }

    #[test]
    fn a_decision_chips_the_line_it_starts_on() {
        let mut proposal = proposal();
        let chunk = Chunk::new(
            "Checks appear where the reader is",
            Some("README.md"),
            "the title said demo",
            "the title says what it is",
            "rename it in place",
            None,
        )
        .unwrap()
        .anchored(crate::fixtures::anchor("README.md", Side::New, 2, 3));
        proposal.apply(Record::Chunk(chunk)).unwrap();
        let page = page(&proposal, &patch());
        assert_eq!(row(&page, 0, 2).decisions(), &[1]);
        assert_eq!(row(&page, 0, 3).decisions(), &[] as &[u32]);
        assert_eq!(page.decisions()[0].row(), Some("L-0-2"));
        assert_eq!(page.decisions()[0].surface(), None);
    }

    #[test]
    fn rationale_marks_the_line_it_ends_on() {
        let mut proposal = proposal();
        let head = proposal.head().sha().clone();
        let why = Rationale::new(
            head,
            crate::fixtures::anchor("cmd/githerb/extra.go", Side::New, 3, 3),
            "kept for the demo",
            author("leandro"),
            at(20),
        )
        .unwrap();
        proposal.apply(Record::Rationale(why)).unwrap();
        let page = page(&proposal, &patch());
        assert_eq!(row(&page, 1, 2).why(), &["kept for the demo".to_owned()]);
    }

    // --- the frame around the diff ---

    #[test]
    fn a_required_check_that_never_ran_is_missing() {
        let proposal = proposal();
        let required = vec![review::CheckName::parse("gate").unwrap()];
        let page = Page::build(&proposal, &patch(), &required, None);
        assert_eq!(page.checks().len(), 1);
        assert_eq!(page.checks()[0].state(), CheckState::Missing);
        assert_eq!(
            page.blocked(),
            Some("a required check has not run on the head revision: gate")
        );
        assert!(!page.landable(), "a missing check blocks");
    }

    #[test]
    fn an_open_note_is_what_blocks_the_gate() {
        let mut proposal = proposal();
        let head = proposal.head().sha().clone();
        note(&mut proposal, &head, "README.md", Side::New, 1, 1, "no");
        let page = page(&proposal, &patch());
        assert_eq!(page.open_notes(), 1);
        assert!(!page.landable(), "an open note blocks");
    }

    #[test]
    fn the_origins_strip_offers_the_base_and_every_revision_but_the_head() {
        let mut proposal = proposal();
        proposal.add_revision(sha('c')).unwrap();
        proposal.add_revision(sha('d')).unwrap();
        let page = Page::build(&proposal, &patch(), &[], Some(2));
        let strip: Vec<(&str, u32, bool)> = page
            .origins()
            .iter()
            .map(|origin| (origin.label(), origin.since(), origin.active()))
            .collect();
        assert_eq!(
            strip,
            vec![("base", 0, false), ("r1", 1, false), ("r2", 2, true)]
        );
        assert_eq!(page.head(), 3);
    }

    // --- how much of a diff is handed over at once ---

    /// A diff of one file per pair, each with that many added lines.
    fn wide(files: &[(&str, usize)]) -> String {
        let mut diff = String::new();
        for (path, lines) in files {
            let _ = writeln!(diff, "diff --git a/{path} b/{path}");
            diff.push_str("new file mode 100644\n--- /dev/null\n");
            let _ = writeln!(diff, "+++ b/{path}\n@@ -0,0 +1,{lines} @@");
            for line in 0..*lines {
                let _ = writeln!(diff, "+line {line}");
            }
        }
        diff
    }

    #[test]
    fn a_small_diff_is_handed_over_whole() {
        let patch = patch::parse(&wide(&[("big.txt", 900), ("small.txt", 10)])).unwrap();
        let page = page(&proposal(), &patch);
        assert_eq!(
            page.files()
                .iter()
                .map(FileView::collapsed)
                .collect::<Vec<bool>>(),
            vec![false, false]
        );
    }

    #[test]
    fn a_large_diff_folds_its_large_files_and_keeps_the_small_ones() {
        let patch = patch::parse(&wide(&[
            ("big.txt", 2900),
            ("small.txt", 200),
            ("mid.txt", 401),
        ]))
        .unwrap();
        let page = page(&proposal(), &patch);
        assert_eq!(
            page.files()
                .iter()
                .map(FileView::collapsed)
                .collect::<Vec<bool>>(),
            vec![true, false, true]
        );
    }

    #[test]
    fn a_page_of_nothing_still_builds() {
        let page = page(&proposal(), &patch::Patch::default());
        assert_eq!(page.files().len(), 0);
        assert_eq!(page.added(), 0);
        assert_eq!(page.threads().len(), 0);
    }

    #[test]
    fn the_diff_counts_are_the_patch_counts() {
        let page = page(&proposal(), &patch());
        assert_eq!((page.added(), page.removed()), (4, 1));
        assert_eq!(page.files()[0].lines(), 5);
        assert_eq!(DIFF.lines().count(), 19);
    }

    #[test]
    fn two_notes_on_one_line_both_render_there() {
        let mut proposal = proposal();
        let head = proposal.head().sha().clone();
        note(&mut proposal, &head, "README.md", Side::New, 1, 1, "first");
        let second = Comment::new(
            head,
            crate::fixtures::anchor("README.md", Side::New, 1, 1),
            "second",
            author("agent"),
            at(48),
        )
        .unwrap();
        proposal.apply(Record::Comment(second)).unwrap();
        let page = page(&proposal, &patch());
        assert_eq!(row(&page, 0, 1).threads(), &[0, 1]);
    }
}
