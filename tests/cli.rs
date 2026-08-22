//! The binary, run the way a person runs it.
//!
//! These are about the shell contract and nothing else: what lands on stdout,
//! what lands on stderr, and what the exit code says. The behaviour behind
//! each verb is tested in `app`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A repository with one proposal open on a side branch, thrown away when the
/// test ends.
struct Repo {
    home: PathBuf,
    root: PathBuf,
}

impl Repo {
    fn new() -> Repo {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let home = std::env::temp_dir().join(format!("githerb-cli-{}-{id}", std::process::id()));
        fs::create_dir_all(&home).unwrap();
        let home = home.canonicalize().unwrap();
        let root = home.join("repo");
        fs::create_dir_all(&root).unwrap();

        let repo = Repo { home, root };
        repo.git(&["init", "-q", "-b", "main"]);
        repo.git(&["config", "user.name", "test"]);
        repo.git(&["config", "user.email", "test@githerb"]);
        repo.git(&["commit", "-q", "--allow-empty", "-m", "root"]);
        repo.git(&["checkout", "-q", "-b", "gate"]);
        repo.git(&["commit", "-q", "--allow-empty", "-m", "the work"]);

        repo
    }

    fn git(&self, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .output()
            .unwrap();

        assert!(output.status.success(), "git {}", args.join(" "));
    }

    /// Run the binary as somebody, or as nobody in particular.
    fn githerb(&self, args: &[&str], author: Option<&str>) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_githerb"));
        command.args(args).current_dir(&self.root);

        match author {
            Some(name) => command.env("GITHERB_AUTHOR", name),
            None => command.env_remove("GITHERB_AUTHOR"),
        };

        command.output().unwrap()
    }

    fn stdout(&self, args: &[&str], author: Option<&str>) -> String {
        let output = self.githerb(args, author);

        assert!(
            output.status.success(),
            "githerb {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );

        String::from_utf8(output.stdout).unwrap()
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ignored = fs::remove_dir_all(&self.home);
    }
}

#[test]
fn a_proposal_is_opened_and_listed_and_shown() {
    let repo = Repo::new();

    let opened = repo.stdout(
        &["propose", "--onto", "main", "--title", "Land the gate"],
        Some("ada"),
    );
    let id = opened.split_whitespace().next().unwrap().to_owned();

    assert!(opened.ends_with("  revision 1  onto main\n"), "{opened}");
    assert!(id.starts_with("land-the-gate-"), "{id}");

    let listed = repo.stdout(&["list"], Some("ada"));
    assert!(listed.starts_with(&id), "{listed}");
    assert!(
        listed.contains("open      r1   0 open  no checks onto main\n"),
        "{listed}"
    );

    let shown = repo.stdout(&["show", &id], Some("ada"));
    assert!(
        shown.starts_with(&format!("{id}\nLand the gate\n\nonto main")),
        "{shown}"
    );
    assert!(
        shown.ends_with("no agent on it\n\nnothing open\n"),
        "{shown}"
    );
}

#[test]
fn a_note_is_signed_by_whoever_the_environment_says() {
    let repo = Repo::new();
    let opened = repo.stdout(&["propose", "--title", "Land the gate"], Some("ada"));
    let id = opened.split_whitespace().next().unwrap().to_owned();

    repo.stdout(
        &[
            "comment",
            &id,
            "--file",
            "a.txt",
            "--line",
            "2:3",
            "--body",
            "this leaks",
        ],
        Some("ada"),
    );

    let json = repo.stdout(&["comments", &id, "--json"], Some("ada"));
    assert!(json.contains(r#""author":"ada""#), "{json}");

    // With nothing in the environment it is whoever git was told about.
    repo.stdout(
        &[
            "comment", &id, "--file", "a.txt", "--line", "9", "--body", "and this",
        ],
        None,
    );

    let json = repo.stdout(&["comments", &id, "--json"], None);
    assert!(json.contains(r#""author":"test""#), "{json}");
    assert_eq!(json.lines().count(), 2);
}

#[test]
fn a_refusal_says_githerb_and_leaves_with_one() {
    let repo = Repo::new();

    let output = repo.githerb(&["show", "nothing-0000000"], Some("ada"));

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "githerb: proposal nothing-0000000: not found\n"
    );
}

#[test]
fn a_command_line_that_does_not_say_what_to_do_leaves_with_two() {
    let repo = Repo::new();

    assert_eq!(repo.githerb(&[], None).status.code(), Some(2));
    assert_eq!(repo.githerb(&["nonsense"], None).status.code(), Some(2));
    assert_eq!(
        repo.githerb(
            &[
                "comment", "x", "--file", "a", "--line", "nope", "--body", "b"
            ],
            None
        )
        .status
        .code(),
        Some(2)
    );
    assert_eq!(repo.githerb(&["describe"], None).status.code(), Some(2));
}

#[test]
fn help_and_version_ask_for_no_repository() {
    let repo = Repo::new();

    let help = repo.githerb(&["--help"], None);
    assert_eq!(help.status.code(), Some(0));
    assert!(
        String::from_utf8(help.stdout)
            .unwrap()
            .contains("proposals are refs under refs/githerb/proposals")
    );

    assert_eq!(
        repo.stdout(&["version"], None),
        format!("githerb {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert!(
        repo.stdout(&["describe", "--template"], None)
            .starts_with("{\n  \"chunks\"")
    );
}
