//! Running the repository's agent, and listening to what it says.
//!
//! githerb never learns what an agent is. The command comes from
//! `.githerb.toml` the same way a check command does, it runs under `sh -c` in
//! a throwaway worktree, and the brief goes in on stdin. What comes back is
//! one line: the last thing it said, cut to what a record can hold.
//!
//! stdout and stderr are merged, because an agent that explains itself on
//! stderr is still explaining itself. Both are drained by their own thread, so
//! a chatty agent cannot deadlock on a full pipe while we wait for it to exit.

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::error::Error;

/// How often the child is asked whether it is done, and therefore how long a
/// stop takes to reach it. There is nothing to wait on: a process exit is not
/// a channel.
const POLL: Duration = Duration::from_millis(100);

/// How much of the last line a one line record can hold.
const SAID_CEILING: usize = 120;

/// The command this repository answers its log with.
#[derive(Debug, Clone)]
pub struct Agent {
    command: String,
}

impl Agent {
    /// Read the command the repository declares.
    ///
    /// # Errors
    ///
    /// [`Error::NoAgent`] when it declares none. A blank command is not an
    /// agent that does nothing, it is a repository that never asked for one.
    pub fn new(command: &str) -> Result<Agent, Error> {
        let command = command.trim();

        if command.is_empty() {
            return Err(Error::NoAgent);
        }

        Ok(Agent {
            command: command.to_owned(),
        })
    }

    /// What the repository declared, for a message that has to name it.
    #[must_use]
    pub fn command(&self) -> &str {
        &self.command
    }

    /// Run the agent in `cwd` with `brief` on its stdin, and hand back the
    /// last thing it said.
    ///
    /// `shutdown` is checked while the child runs: when it flips, the child is
    /// killed and this returns [`Error::Stopped`]. Only `sh` is killed, so a
    /// grandchild that put itself in another process group outlives us; there
    /// is no portable way to reach it and the job is over either way.
    ///
    /// # Errors
    ///
    /// [`Error::AgentStopped`] when it exits non-zero, carrying the last line
    /// it said; [`Error::Stopped`] when the runner was asked to stop;
    /// [`Error::Io`] when the child could not be started or waited on.
    pub fn call(
        &self,
        cwd: &Path,
        brief: &str,
        env: &[(&str, &str)],
        shutdown: &AtomicBool,
    ) -> Result<String, Error> {
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(&self.command)
            .current_dir(cwd)
            .envs(env.iter().copied())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        feed(&mut child, brief);

        let output = Arc::new(Mutex::new(Vec::new()));
        let readers = drain(&mut child, &output);

        let Some(status) = wait(&mut child, shutdown)? else {
            return Err(Error::Stopped);
        };

        // Safe to wait on now: the child is gone, so both pipes are closed and
        // both threads are on their way out.
        for reader in readers {
            let _ = reader.join();
        }

        let said = tail(&output.lock().unwrap_or_else(PoisonError::into_inner));

        if status.success() {
            return Ok(said);
        }

        Err(Error::AgentStopped(said))
    }
}

/// Hand the brief over on stdin, from a thread, and close the pipe after.
///
/// An agent that never reads its stdin would otherwise block us the moment the
/// brief outgrows the pipe buffer.
fn feed(child: &mut Child, brief: &str) {
    let Some(mut pipe) = child.stdin.take() else {
        return;
    };

    let brief = brief.to_owned();

    // The write fails when the agent exits without reading, which is the
    // agent's business and not a failure of the job.
    thread::spawn(move || {
        let _ = pipe.write_all(brief.as_bytes());
    });
}

/// Read both streams into one buffer, in the order the bytes arrive.
fn drain(child: &mut Child, into: &Arc<Mutex<Vec<u8>>>) -> Vec<JoinHandle<()>> {
    let mut readers = Vec::with_capacity(2);

    if let Some(stream) = child.stdout.take() {
        readers.push(copy(stream, Arc::clone(into)));
    }

    if let Some(stream) = child.stderr.take() {
        readers.push(copy(stream, Arc::clone(into)));
    }

    readers
}

/// One stream, appended to the shared buffer as it comes.
fn copy(mut source: impl Read + Send + 'static, into: Arc<Mutex<Vec<u8>>>) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut chunk = [0_u8; 4096];

        loop {
            match source.read(&mut chunk) {
                Ok(0) | Err(_) => return,
                Ok(read) => into
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .extend_from_slice(&chunk[..read]),
            }
        }
    })
}

/// Wait for the child, or kill it when the runner is stopped.
///
/// [`None`] is the stop: the child was killed and reaped, and there is nothing
/// worth reading in what it half said.
fn wait(child: &mut Child, shutdown: &AtomicBool) -> Result<Option<ExitStatus>, Error> {
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }

        if shutdown.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();

            return Ok(None);
        }

        thread::sleep(POLL);
    }
}

/// The last thing the agent said, cut to what a one line record holds.
fn tail(output: &[u8]) -> String {
    String::from_utf8_lossy(output)
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.chars().take(SAID_CEILING).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;
    use std::sync::mpsc;
    use std::time::Instant;

    /// A shutdown flag nobody has flipped.
    fn running() -> AtomicBool {
        AtomicBool::new(false)
    }

    /// The pid the agent wrote down as it started, once it is there.
    fn started(marker: &Path) -> u32 {
        let deadline = Instant::now() + Duration::from_secs(10);

        loop {
            if let Ok(text) = std::fs::read_to_string(marker)
                && let Ok(pid) = text.trim().parse::<u32>()
            {
                return pid;
            }

            assert!(Instant::now() < deadline, "the agent never started");
            thread::sleep(Duration::from_millis(5));
        }
    }

    /// Whether that process is still around. The child is reaped before the
    /// call returns, so a live pid here is one nothing killed.
    fn alive(pid: u32) -> bool {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    // --- what the agent is given ---

    #[test]
    fn an_agent_reads_the_brief_on_stdin_and_the_environment_it_was_given() -> Result<(), Error> {
        let scratch = Scratch::new("agent-brief");
        let agent = Agent::new("cat > brief.txt; printf '%s\\n' \"$GITHERB_ANSWERS\"")?;

        let said = agent.call(
            scratch.path(),
            "Answer every note below\n",
            &[("GITHERB_ANSWERS", "/tmp/answers.jsonl")],
            &running(),
        )?;

        assert_eq!(said, "/tmp/answers.jsonl");
        assert_eq!(
            std::fs::read_to_string(scratch.path().join("brief.txt"))?,
            "Answer every note below\n"
        );
        Ok(())
    }

    #[test]
    fn a_repository_that_declares_no_agent_is_refused() {
        assert!(matches!(Agent::new("   "), Err(Error::NoAgent)));
    }

    // --- what the agent says ---

    #[test]
    fn what_the_agent_said_last_is_what_the_record_carries() -> Result<(), Error> {
        let scratch = Scratch::new("agent-tail");
        let agent = Agent::new("echo working; echo 'named the second line'; echo")?;

        let said = agent.call(scratch.path(), "", &[], &running())?;

        assert_eq!(said, "named the second line");
        Ok(())
    }

    #[test]
    fn an_agent_that_explains_itself_on_stderr_is_still_heard() -> Result<(), Error> {
        let scratch = Scratch::new("agent-stderr");
        let agent = Agent::new("echo 'no api key' >&2")?;

        let said = agent.call(scratch.path(), "", &[], &running())?;

        assert_eq!(said, "no api key");
        Ok(())
    }

    #[test]
    fn a_long_last_line_is_cut_to_what_a_record_holds() -> Result<(), Error> {
        let scratch = Scratch::new("agent-long");
        let agent = Agent::new("printf 'x%.0s' $(seq 1 400); echo")?;

        let said = agent.call(scratch.path(), "", &[], &running())?;

        assert_eq!(said, "x".repeat(SAID_CEILING));
        Ok(())
    }

    #[test]
    fn an_agent_that_says_nothing_says_nothing() -> Result<(), Error> {
        let scratch = Scratch::new("agent-quiet");
        let agent = Agent::new("exit 0")?;

        let said = agent.call(scratch.path(), "", &[], &running())?;

        assert_eq!(said, "");
        Ok(())
    }

    #[test]
    fn an_agent_that_stopped_carries_the_reason_it_gave() -> Result<(), Error> {
        let scratch = Scratch::new("agent-failed");
        let agent = Agent::new("echo trying; echo 'the model refused'; exit 3")?;

        let refused = agent.call(scratch.path(), "", &[], &running());

        match refused {
            Err(Error::AgentStopped(said)) => assert_eq!(said, "the model refused"),
            other => panic!("{other:?}"),
        }
        Ok(())
    }

    // --- stopping ---

    #[test]
    fn an_agent_still_running_when_the_runner_stops_is_killed() -> Result<(), Error> {
        let scratch = Scratch::new("agent-killed");
        let marker = scratch.path().join("running");
        let agent = Agent::new(&format!("echo $$ > '{}'; sleep 30", marker.display()))?;
        let shutdown = Arc::new(AtomicBool::new(false));

        let (tell, told) = mpsc::channel();
        let cwd = scratch.path().to_path_buf();
        let flag = Arc::clone(&shutdown);
        let worker = thread::spawn(move || {
            let _ = tell.send(());
            agent.call(&cwd, "", &[], &flag)
        });

        // The child has to exist before the stop means anything, and only the
        // child can say so: it writes down its own pid as it starts.
        told.recv().unwrap();
        let pid = started(&marker);

        let began = Instant::now();
        shutdown.store(true, Ordering::Relaxed);
        let outcome = worker.join().unwrap();

        assert!(matches!(outcome, Err(Error::Stopped)), "{outcome:?}");
        assert!(
            began.elapsed() < Duration::from_secs(5),
            "it waited the sleep out"
        );
        assert!(!alive(pid), "the agent is still running");
        Ok(())
    }
}
