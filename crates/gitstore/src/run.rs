//! Running the git binary.
//!
//! Every call in this crate lands here: one `Command`, an explicit look at the
//! exit status, and stderr carried into the error. Nothing goes through a
//! shell, so no argument ever needs quoting.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use crate::error::Error;

/// The exit code git uses to answer "no" rather than to fail: not an ancestor,
/// not set in the config. Any other non-zero exit is a real refusal.
pub(crate) const ANSWER_IS_NO: i32 = 1;

/// Run git in `dir` and hand back whatever it did, refusal included.
pub(crate) fn capture(dir: &Path, args: &[&str], stdin: Option<&str>) -> Result<Output, Error> {
    let mut command = Command::new("git");
    command.args(args).current_dir(dir);

    let Some(input) = stdin else {
        return Ok(command.output()?);
    };

    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // The writer gets its own thread. git answers while it reads, and one
    // thread doing both deadlocks the moment either pipe fills, which for
    // `cat-file --batch` is a few thousand object names in.
    if let Some(mut pipe) = child.stdin.take() {
        let input = input.to_owned();
        std::thread::spawn(move || pipe.write_all(input.as_bytes()));
    }

    Ok(child.wait_with_output()?)
}

/// The bytes git wrote on stdout, or the reason it refused.
pub(crate) fn stdout_of(dir: &Path, args: &[&str], stdin: Option<&str>) -> Result<Vec<u8>, Error> {
    let output = capture(dir, args, stdin)?;

    if output.status.success() {
        return Ok(output.stdout);
    }

    Err(refused(args, &output.stderr))
}

/// The bytes git wrote, or `None` when it exited with `tolerated` instead.
pub(crate) fn stdout_or(
    dir: &Path,
    args: &[&str],
    tolerated: i32,
) -> Result<Option<Vec<u8>>, Error> {
    let output = capture(dir, args, None)?;

    if output.status.success() {
        return Ok(Some(output.stdout));
    }

    if output.status.code() == Some(tolerated) {
        return Ok(None);
    }

    Err(refused(args, &output.stderr))
}

/// git's output as text, without the trailing newlines it always adds.
pub(crate) fn trimmed(bytes: Vec<u8>) -> Result<String, Error> {
    let mut text = String::from_utf8(bytes).map_err(|_| Error::Utf8)?;

    while text.ends_with('\n') {
        text.pop();
    }

    Ok(text)
}

/// A path as an argument, which git only accepts as text.
pub(crate) fn arg(path: &Path) -> Result<&str, Error> {
    path.to_str().ok_or(Error::Utf8)
}

/// The refusal, naming the argv that caused it and what git said about it.
pub(crate) fn refused(args: &[&str], stderr: &[u8]) -> Error {
    Error::Git {
        args: args.join(" "),
        stderr: String::from_utf8_lossy(stderr).trim().to_owned(),
    }
}
