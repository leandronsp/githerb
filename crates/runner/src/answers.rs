//! The file an agent answers in.
//!
//! An agent that changed code has said something to the person who asked, and
//! stdout is not where that belongs: it scrolls past, it is not a record, and
//! by the time anybody reads it the agent is gone. So the runner makes a file,
//! names it in the environment as `GITHERB_ANSWERS`, and reads it back into
//! replies afterwards.
//!
//! It lives in the temp directory rather than in the worktree, so an agent
//! that commits everything it touched cannot commit it.
//!
//! One line of JSON per answer:
//!
//! ```text
//! {"note":"<comment id>","say":"<one line>"}
//! ```
//!
//! A line this build cannot read is reported and skipped, never fatal. The
//! agent is outside code we control, and one bad line must not lose the good
//! ones beside it.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::process;

use serde_json::{Map, Value};

use crate::error::Error;

/// What an agent said about one note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    note: String,
    say: String,
}

impl Answer {
    /// The only way to build one.
    #[must_use]
    pub fn new(note: &str, say: &str) -> Self {
        Self {
            note: note.to_owned(),
            say: say.to_owned(),
        }
    }

    /// The note it answers, as the agent named it. Nothing here says the note
    /// exists: the core refuses an answer to a note nobody wrote.
    #[must_use]
    pub fn note(&self) -> &str {
        &self.note
    }

    /// What it said.
    #[must_use]
    pub fn say(&self) -> &str {
        &self.say
    }
}

/// The file one job's agent writes its answers into, removed when it is
/// dropped.
#[derive(Debug)]
pub struct AnswersFile {
    path: PathBuf,
}

impl AnswersFile {
    /// Make an empty file for this proposal, so the agent appends to something
    /// that is already there.
    ///
    /// # Errors
    ///
    /// The temp directory cannot be written to.
    pub fn create(proposal_id: &str) -> Result<AnswersFile, Error> {
        let path = std::env::temp_dir().join(format!(
            "githerb-answers-{}-{}.jsonl",
            safe(proposal_id),
            process::id()
        ));

        File::create(&path)?;

        Ok(AnswersFile { path })
    }

    /// Where it is, which is what the agent is told.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for AnswersFile {
    fn drop(&mut self) {
        // Nobody to tell inside a drop, and a file left in the temp directory
        // is the smallest failure available here.
        let _ = fs::remove_file(&self.path);
    }
}

/// Read back what the agent wrote: the answers, and the lines this build could
/// not make sense of.
///
/// A file that is not there is no answers rather than a failure: an agent that
/// changed code without saying anything is a case the runner handles on its
/// own, and it may well have deleted the file.
///
/// # Errors
///
/// The file exists and cannot be read.
pub fn read_answers(path: &Path) -> Result<(Vec<Answer>, Vec<String>), Error> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
            return Ok((Vec::new(), Vec::new()));
        }
        Err(cause) => return Err(Error::Io(cause)),
    };

    let mut answers = Vec::new();
    let mut unreadable = Vec::new();

    for line in text.lines() {
        let line = line.trim();

        if line.is_empty() {
            continue;
        }

        match parse(line) {
            Some(answer) => answers.push(answer),
            None => unreadable.push(line.to_owned()),
        }
    }

    Ok((answers, unreadable))
}

/// One line, if it is an object whose `note` and `say` are text.
fn parse(line: &str) -> Option<Answer> {
    let value: Value = serde_json::from_str(line).ok()?;
    let object = value.as_object()?;

    Some(Answer {
        note: field(object, "note")?,
        say: field(object, "say")?,
    })
}

/// One field of the object: absent reads as blank, and the core refuses it
/// later with a sentence about what was missing. Present but not text is a
/// line this build cannot read.
fn field(object: &Map<String, Value>, name: &str) -> Option<String> {
    let Some(value) = object.get(name) else {
        return Some(String::new());
    };

    value.as_str().map(str::to_owned)
}

/// A proposal id as a file name. A slash would make a directory nobody
/// created, and a dot would hide the extension.
fn safe(id: &str) -> String {
    id.chars()
        .map(|letter| match letter {
            '/' | '\\' | '.' => '-',
            other => other,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// The answers file, with these lines already in it.
    fn written(id: &str, lines: &str) -> Result<AnswersFile, Error> {
        let file = AnswersFile::create(id)?;
        let mut open = fs::OpenOptions::new().append(true).open(file.path())?;
        open.write_all(lines.as_bytes())?;

        Ok(file)
    }

    // --- the file ---

    #[test]
    fn the_file_is_made_empty_and_named_after_the_proposal() -> Result<(), Error> {
        let file = AnswersFile::create("land-the-gate")?;

        let name = file
            .path()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();

        assert_eq!(
            name,
            format!("githerb-answers-land-the-gate-{}.jsonl", process::id())
        );
        assert_eq!(fs::read_to_string(file.path())?, "");
        Ok(())
    }

    #[test]
    fn an_id_that_could_name_a_directory_cannot() {
        assert_eq!(safe("a/b.c"), "a-b-c");
    }

    #[test]
    fn the_file_goes_away_with_the_job() -> Result<(), Error> {
        let file = AnswersFile::create("gone")?;
        let path = file.path().to_path_buf();

        drop(file);

        assert!(!path.exists());
        Ok(())
    }

    #[test]
    fn the_file_is_not_in_the_worktree_it_would_be_committed_from() -> Result<(), Error> {
        let file = AnswersFile::create("outside")?;

        assert_eq!(file.path().parent(), Some(std::env::temp_dir().as_path()));
        Ok(())
    }

    // --- reading it back ---

    #[test]
    fn every_answer_is_read_and_the_blank_lines_are_not() -> Result<(), Error> {
        let file = written(
            "read-answers",
            "{\"note\":\"9b052da286a4\",\"say\":\"renamed it\"}\n\n  \n{\"note\":\"3926a440d610\",\"say\":\"left it alone\"}\n",
        )?;

        let (answers, unreadable) = read_answers(file.path())?;

        assert_eq!(
            answers,
            vec![
                Answer::new("9b052da286a4", "renamed it"),
                Answer::new("3926a440d610", "left it alone"),
            ]
        );
        assert_eq!(unreadable, Vec::<String>::new());
        Ok(())
    }

    #[test]
    fn a_line_this_build_cannot_read_is_reported_and_the_rest_still_land() -> Result<(), Error> {
        let file = written(
            "unreadable",
            "not json at all\n{\"note\":\"9b052da286a4\",\"say\":\"renamed it\"}\n[1,2]\n{\"note\":7}\n",
        )?;

        let (answers, unreadable) = read_answers(file.path())?;

        assert_eq!(answers, vec![Answer::new("9b052da286a4", "renamed it")]);
        assert_eq!(unreadable, vec!["not json at all", "[1,2]", "{\"note\":7}"]);
        Ok(())
    }

    #[test]
    fn a_missing_field_reads_as_blank_and_is_refused_further_down() -> Result<(), Error> {
        let file = written("missing-field", "{\"say\":\"renamed it\"}\n")?;

        let (answers, unreadable) = read_answers(file.path())?;

        assert_eq!(answers, vec![Answer::new("", "renamed it")]);
        assert_eq!(unreadable, Vec::<String>::new());
        Ok(())
    }

    #[test]
    fn a_file_the_agent_removed_is_no_answers() -> Result<(), Error> {
        let (answers, unreadable) =
            read_answers(&std::env::temp_dir().join("githerb-answers-nobody-wrote.jsonl"))?;

        assert_eq!(answers, Vec::<Answer>::new());
        assert_eq!(unreadable, Vec::<String>::new());
        Ok(())
    }
}
