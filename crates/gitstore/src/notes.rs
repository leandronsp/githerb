//! Notes as the append-only log, read in two processes however long it is.
//!
//! `notes list` gives every (note blob, object) pair the ref carries, and one
//! `cat-file --batch` reads every one of those blobs. The cost of reading the
//! whole log is therefore two processes, not one per revision, which is the
//! reason this crate exists in the shape it does.

use std::collections::HashMap;

use crate::error::Error;
use crate::git::Repo;
use crate::run::stdout_of;

impl Repo {
    /// Every (note blob, annotated object) pair a notes ref carries.
    ///
    /// A notes ref nobody has written to yet is an empty log, not a failure.
    pub fn note_list(&self, notes_ref: &str) -> Result<Vec<(String, String)>, Error> {
        let selector = format!("--ref={notes_ref}");

        match self.run(&["notes", &selector, "list"]) {
            Ok(listing) => Ok(columns(&listing)),
            Err(err) => self.empty_if_never_written(notes_ref, err),
        }
    }

    /// Read several objects in a single `cat-file --batch`.
    ///
    /// An object git reports as `missing` is left out of the map rather than
    /// failing the batch: a note blob can be garbage collected out from under
    /// a listing, and the rest of the log is still readable.
    pub fn cat_blobs(&self, shas: &[&str]) -> Result<HashMap<String, String>, Error> {
        if shas.is_empty() {
            return Ok(HashMap::new());
        }

        let mut input = shas.join("\n");
        input.push('\n');

        let batch = stdout_of(self.root(), &["cat-file", "--batch"], Some(&input))?;

        unbatch(&batch)
    }

    /// Append one line to the note on `object`.
    ///
    /// `--no-separator` is what makes the note a log: the new line follows the
    /// last one with nothing between them.
    pub fn note_append(&self, notes_ref: &str, object: &str, line: &str) -> Result<(), Error> {
        let selector = format!("--ref={notes_ref}");

        self.run(&[
            "notes",
            &selector,
            "append",
            "--no-separator",
            "-m",
            line,
            object,
        ])?;

        Ok(())
    }

    /// The whole notes ref as object -> note text, in two processes.
    pub fn notes(&self, notes_ref: &str) -> Result<HashMap<String, String>, Error> {
        let listed = self.note_list(notes_ref)?;
        let blobs: Vec<&str> = listed.iter().map(|(blob, _)| blob.as_str()).collect();
        let texts = self.cat_blobs(&blobs)?;

        Ok(listed
            .into_iter()
            // Two objects whose notes read the same share one blob, so this
            // reads rather than takes.
            .filter_map(|(blob, object)| texts.get(&blob).map(|text| (object, text.clone())))
            .collect())
    }

    /// git 2.51 answers an unwritten notes ref with an empty list. Older ones
    /// refuse, so a refusal is checked against the ref before it is believed.
    fn empty_if_never_written(
        &self,
        notes_ref: &str,
        err: Error,
    ) -> Result<Vec<(String, String)>, Error> {
        let full = format!("refs/notes/{notes_ref}");

        if self
            .run(&["rev-parse", "--verify", "--quiet", &full])
            .is_err()
        {
            return Ok(Vec::new());
        }

        Err(err)
    }
}

/// Split `notes list` output into its two columns.
fn columns(listing: &str) -> Vec<(String, String)> {
    listing
        .lines()
        .filter_map(|line| line.split_once(' '))
        .map(|(blob, object)| (blob.to_owned(), object.to_owned()))
        .collect()
}

/// Parse `cat-file --batch` framing: `<oid> <type> <size>\n<content>\n` per
/// object, or `<name> missing\n` for one git could not find.
fn unbatch(batch: &[u8]) -> Result<HashMap<String, String>, Error> {
    let mut objects = HashMap::new();
    let mut rest = batch;

    while !rest.is_empty() {
        let Some(end) = rest.iter().position(|byte| *byte == b'\n') else {
            break;
        };

        let header = std::str::from_utf8(&rest[..end]).map_err(|_| Error::Utf8)?;
        rest = &rest[end + 1..];

        let Some((name, size)) = framing(header) else {
            continue;
        };

        if rest.len() < size {
            break;
        }

        let content = String::from_utf8(rest[..size].to_vec()).map_err(|_| Error::Utf8)?;
        objects.insert(name.to_owned(), content);

        rest = &rest[size..];

        if rest.first() == Some(&b'\n') {
            rest = &rest[1..];
        }
    }

    Ok(objects)
}

/// The object name and byte count in one batch header, or `None` when git
/// answered `missing`.
fn framing(header: &str) -> Option<(&str, usize)> {
    let mut fields = header.split(' ');
    let name = fields.next()?;
    let kind = fields.next()?;

    if kind == "missing" {
        return None;
    }

    Some((name, fields.next()?.parse().ok()?))
}
