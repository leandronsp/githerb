//! Running what the repository declares against the head revision, and
//! writing down what happened.
//!
//! Every check runs in a throwaway worktree of that exact commit, never in
//! your working tree, so the answer is about the code that would land rather
//! than the code you happen to have open, and you can keep editing while it
//! runs.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use review::{Author, Check, CheckName, CheckStatus, ProposalId, Record, Sha, Timestamp};

use crate::config::{Config, FILE};
use crate::error::{Error, Result};
use crate::format;
use crate::store::Store;

/// Makes each worktree directory of one process its own.
static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Run every declared check that has not already answered for this revision,
/// and write each result down as it lands.
///
/// A revision that already answered is not asked twice: the commit is the
/// same commit, and nothing about it changed while you were away. Results are
/// written to `out` in the order they were declared, so a long gate reads as
/// it goes.
///
/// # Errors
///
/// A proposal nobody opened, a worktree git refused to make, or a check that
/// was killed rather than answered.
pub fn check(
    store: &Store,
    config: &Config,
    author: &Author,
    now: Timestamp,
    id: &ProposalId,
    out: &mut dyn Write,
) -> Result<Vec<Check>> {
    let proposal = store.load(id)?;
    let head = proposal.head().sha().clone();
    let already = proposal.checks();

    if config.checks().is_empty() {
        writeln!(out, "no checks declared in {FILE}")?;

        return Ok(Vec::new());
    }

    let mut results = Vec::new();

    for (name, command) in config.checks() {
        let name = CheckName::parse(name)?;

        let result = if let Some(done) = already.iter().find(|done| done.name() == &name) {
            (*done).clone()
        } else {
            let ran = one(store, &name, command, &head, author, now)?;
            store.annotate(&head, &Record::Check(ran.clone()))?;
            ran
        };

        writeln!(out, "{}", format::check_line(&result))?;
        results.push(result);
    }

    Ok(results)
}

/// How many of a set of results said no.
#[must_use]
pub fn refused(results: &[Check]) -> usize {
    results.iter().filter(|result| !result.passed()).count()
}

/// The checks the repository requires, as names the gate can ask for.
///
/// # Errors
///
/// A check declared under a blank name.
pub fn required(config: &Config) -> Result<Vec<CheckName>> {
    config
        .required()
        .into_iter()
        .map(|name| Ok(CheckName::parse(name)?))
        .collect()
}

/// One check, in a worktree of its own.
fn one(
    store: &Store,
    name: &CheckName,
    command: &str,
    head: &Sha,
    author: &Author,
    now: Timestamp,
) -> Result<Check> {
    let dir = scratch();
    std::fs::create_dir_all(&dir)?;
    store.repo().worktree_add(&dir, head.as_str())?;

    let began = Instant::now();
    let status = run(command, &dir);

    remove(store, &dir);

    let status = status?;
    let seconds = u32::try_from(began.elapsed().as_secs()).unwrap_or(u32::MAX);

    Ok(Check::new(
        name.clone(),
        status.ok_or_else(|| Error::CheckKilled(name.clone()))?,
        head.clone(),
        seconds,
        author.clone(),
        now,
    ))
}

/// Run the command the repository declares, and read the exit code as a
/// verdict.
///
/// A command that died on a signal was killed, by a Ctrl-C or a shutdown, and
/// killing something is not a verdict on it: that is `None`, and nothing is
/// recorded. The command comes from the repository, which is the same trust
/// you give a Makefile.
fn run(command: &str, dir: &Path) -> Result<Option<CheckStatus>> {
    let status = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(dir)
        .status()?;

    Ok(status.code().map(|code| {
        if code == 0 {
            CheckStatus::Passed
        } else {
            CheckStatus::Failed
        }
    }))
}

/// A directory beside the repository, for one check.
fn scratch() -> PathBuf {
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);

    std::env::temp_dir().join(format!("githerb-check-{}-{id}", std::process::id()))
}

/// Take the worktree away whatever the check did to it. Neither failure is
/// worth losing the verdict over: a leaked directory is swept up by the next
/// prune, and git is told about it either way.
fn remove(store: &Store, dir: &Path) {
    let _ignored = store.repo().worktree_remove(dir);
    let _ignored = std::fs::remove_dir_all(dir);
}
