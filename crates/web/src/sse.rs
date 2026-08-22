//! Server-sent events: the only way this server pushes.
//!
//! The page opens one stream and never polls. A `Sink` is the write half of a
//! connection whose event-stream headers have already gone out; every event is
//! flushed as it is written, because an event that waits for the next one is a
//! page that lags behind the repository.
//!
//! A write that fails means the tab closed. The closure holding the sink is
//! expected to return on it rather than retry: there is nobody there.

use std::io::{self, Write};
use std::net::TcpStream;

/// The status line and headers every event stream opens with.
const HEAD: &[u8] = b"HTTP/1.1 200 OK\r\n\
Content-Type: text/event-stream\r\n\
Cache-Control: no-cache\r\n\
Connection: keep-alive\r\n\
\r\n";

/// The write half of an open event stream.
pub struct Sink {
    out: Box<dyn Write + Send>,
}

impl Sink {
    /// Send one named event.
    ///
    /// Data spanning several lines becomes one `data:` line each, which is how
    /// the browser puts it back together with the newlines intact. A single
    /// trailing newline is dropped rather than sent as an empty last line.
    ///
    /// # Errors
    ///
    /// Fails when the connection is gone.
    pub fn event(&mut self, name: &str, data: &str) -> io::Result<()> {
        let mut frame = String::with_capacity(data.len() + name.len() + 16);
        frame.push_str("event: ");
        frame.push_str(name);
        frame.push('\n');

        for line in lines(data) {
            frame.push_str("data: ");
            frame.push_str(line);
            frame.push('\n');
        }
        frame.push('\n');

        self.out.write_all(frame.as_bytes())?;
        self.out.flush()
    }

    /// Send a comment: bytes the browser ignores.
    ///
    /// This is the heartbeat. It proves the connection is alive, and it is the
    /// cheapest way to find out that it is not.
    ///
    /// # Errors
    ///
    /// Fails when the connection is gone.
    pub fn comment(&mut self, text: &str) -> io::Result<()> {
        self.out.write_all(format!(": {text}\n\n").as_bytes())?;
        self.out.flush()
    }

    /// Answer the request with the event-stream headers and take the socket.
    pub(crate) fn open(stream: &TcpStream) -> io::Result<Self> {
        let mut out = stream.try_clone()?;
        out.write_all(HEAD)?;
        out.flush()?;

        Ok(Self::new(Box::new(out)))
    }

    pub(crate) fn new(out: Box<dyn Write + Send>) -> Self {
        Self { out }
    }
}

fn lines(data: &str) -> impl Iterator<Item = &str> {
    data.strip_suffix('\n')
        .unwrap_or(data)
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct Recorder(Arc<Mutex<Vec<u8>>>);

    impl Write for Recorder {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn recorded() -> (Sink, Arc<Mutex<Vec<u8>>>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        (Sink::new(Box::new(Recorder(Arc::clone(&seen)))), seen)
    }

    fn text(seen: &Arc<Mutex<Vec<u8>>>) -> String {
        String::from_utf8(seen.lock().unwrap().clone()).unwrap()
    }

    // --- event ---

    #[test]
    fn an_event_is_a_name_a_line_and_a_blank_line() -> io::Result<()> {
        let (mut sink, seen) = recorded();

        sink.event("bar", "passing")?;

        assert_eq!(text(&seen), "event: bar\ndata: passing\n\n");
        Ok(())
    }

    #[test]
    fn a_multi_line_payload_becomes_one_data_line_each() -> io::Result<()> {
        let (mut sink, seen) = recorded();

        sink.event("page", "<p>\n  one\n</p>")?;

        assert_eq!(
            text(&seen),
            "event: page\ndata: <p>\ndata:   one\ndata: </p>\n\n"
        );
        Ok(())
    }

    #[test]
    fn a_trailing_newline_does_not_become_an_empty_line() -> io::Result<()> {
        let (mut sink, seen) = recorded();

        sink.event("page", "<p>hi</p>\n")?;

        assert_eq!(text(&seen), "event: page\ndata: <p>hi</p>\n\n");
        Ok(())
    }

    #[test]
    fn an_empty_payload_still_sends_a_data_line() -> io::Result<()> {
        let (mut sink, seen) = recorded();

        sink.event("reload", "")?;

        assert_eq!(text(&seen), "event: reload\ndata: \n\n");
        Ok(())
    }

    // --- comment ---

    #[test]
    fn a_comment_is_a_colon_and_a_blank_line() -> io::Result<()> {
        let (mut sink, seen) = recorded();

        sink.comment("keepalive")?;

        assert_eq!(text(&seen), ": keepalive\n\n");
        Ok(())
    }
}
