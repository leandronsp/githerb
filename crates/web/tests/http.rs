//! The server against a real socket.
//!
//! Every test here binds port 0, speaks raw HTTP over a `TcpStream` and reads
//! the answer to end of connection. Nothing sleeps to synchronise: the server
//! closing the connection is what ends a read, and a channel is what proves
//! `serve` returned.

// A test binary; the crate root allows the same three under `cfg(test)`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use web::{Error, Handler, MAX_BODY, Method, Reply, Request, Response, Server};

/// How long a client waits for an answer before calling the server broken.
const PATIENCE: Duration = Duration::from_secs(5);

/// How long `serve` may take to notice the shutdown flag.
const SHUTDOWN: Duration = Duration::from_millis(500);

// --- the harness ---

/// A server on its own thread, with the flag that stops it.
struct Serving {
    addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    landed: Receiver<Result<(), Error>>,
}

impl Serving {
    fn start(handler: Handler) -> Result<Self, Error> {
        let server = Server::bind("127.0.0.1:0")?;
        let addr = server.addr();
        let shutdown = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&shutdown);
        let (done, landed) = mpsc::channel();

        thread::spawn(move || done.send(server.serve(handler, flag)));

        Ok(Self {
            addr,
            shutdown,
            landed,
        })
    }

    /// Send one raw request and read the answer to end of connection.
    fn ask(&self, raw: &str) -> Result<String, Error> {
        let mut stream = TcpStream::connect(self.addr)?;
        stream.set_read_timeout(Some(PATIENCE))?;
        stream.write_all(raw.as_bytes())?;
        stream.flush()?;

        let mut answer = String::new();
        stream.read_to_string(&mut answer)?;

        Ok(answer)
    }

    /// Set the flag and say whether `serve` returned in time.
    fn stop(&self) -> bool {
        self.shutdown.store(true, Ordering::Relaxed);
        self.landed.recv_timeout(SHUTDOWN).is_ok()
    }
}

impl Drop for Serving {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

fn routes() -> Handler {
    Arc::new(|request: Request| -> Reply {
        match (request.method(), request.path()) {
            (Method::Get | Method::Head, "/hello") => {
                let name = request.query_value("name").unwrap_or("nobody");
                Response::text(format!("hello {name}")).into()
            }
            (Method::Post, "/form") => {
                let body = request.form_value("body").unwrap_or_default();
                let file = request.form_value("file").unwrap_or_default();
                Response::text(format!("{body}|{file}")).into()
            }
            (Method::Post, "/size") => Response::text(request.body().len().to_string()).into(),
            (Method::Get, "/boom") => panic!("this handler is broken"),
            (Method::Get, "/events") => Reply::stream(|sink| {
                sink.comment("open").unwrap();
                sink.event("bar", "passing").unwrap();
                sink.event("page", "<p>\n  one\n</p>").unwrap();
            }),
            _ => Response::not_found().into(),
        }
    })
}

fn split(answer: &str) -> (&str, &str) {
    answer.split_once("\r\n\r\n").unwrap_or((answer, ""))
}

fn status(answer: &str) -> &str {
    answer.lines().next().unwrap_or("")
}

fn header<'a>(answer: &'a str, name: &str) -> Option<&'a str> {
    split(answer)
        .0
        .lines()
        .filter_map(|line| line.split_once(": "))
        .find(|(field, _)| field.eq_ignore_ascii_case(name))
        .map(|(_, value)| value)
}

// --- requests ---

#[test]
fn a_get_reaches_the_handler_with_its_query() -> Result<(), Error> {
    let serving = Serving::start(routes())?;

    let answer = serving.ask("GET /hello?name=ada HTTP/1.1\r\nHost: x\r\n\r\n")?;

    assert_eq!(status(&answer), "HTTP/1.1 200 OK");
    assert_eq!(header(&answer, "content-length"), Some("9"));
    assert_eq!(header(&answer, "Connection"), Some("close"));
    assert_eq!(split(&answer).1, "hello ada");
    Ok(())
}

#[test]
fn a_post_carries_a_decoded_form() -> Result<(), Error> {
    let serving = Serving::start(routes())?;
    let body = "body=needs+a+test&file=crates%2Fweb%2Fsrc%2Fhttp.rs";

    let answer = serving.ask(&format!(
        "POST /form HTTP/1.1\r\n\
         Content-Type: application/x-www-form-urlencoded\r\n\
         Content-Length: {}\r\n\r\n{body}",
        body.len()
    ))?;

    assert_eq!(split(&answer).1, "needs a test|crates/web/src/http.rs");
    Ok(())
}

#[test]
fn an_unknown_path_is_a_404() -> Result<(), Error> {
    let serving = Serving::start(routes())?;

    let answer = serving.ask("GET /nowhere HTTP/1.1\r\n\r\n")?;

    assert_eq!(status(&answer), "HTTP/1.1 404 Not Found");
    assert_eq!(split(&answer).1, "not found");
    Ok(())
}

#[test]
fn a_head_sends_the_headers_and_stops() -> Result<(), Error> {
    let serving = Serving::start(routes())?;

    let answer = serving.ask("HEAD /hello?name=ada HTTP/1.1\r\n\r\n")?;

    assert_eq!(status(&answer), "HTTP/1.1 200 OK");
    assert_eq!(header(&answer, "content-length"), Some("9"));
    assert_eq!(split(&answer).1, "");
    Ok(())
}

#[test]
fn a_request_the_server_cannot_read_is_a_400() -> Result<(), Error> {
    let serving = Serving::start(routes())?;

    let answer = serving.ask("GET\r\n\r\n")?;

    assert_eq!(status(&answer), "HTTP/1.1 400 Bad Request");
    Ok(())
}

// --- ceilings ---

#[test]
fn a_body_over_the_ceiling_is_refused_before_it_arrives() -> Result<(), Error> {
    let serving = Serving::start(routes())?;

    // The ceiling is on the length the client declares, so the megabyte never
    // has to travel and the connection never has to be drained.
    let answer = serving.ask(&format!(
        "POST /size HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
        MAX_BODY + 1
    ))?;

    assert_eq!(status(&answer), "HTTP/1.1 413 Content Too Large");
    assert_eq!(split(&answer).1, "body too large");
    Ok(())
}

#[test]
fn a_body_at_the_ceiling_is_read_whole() -> Result<(), Error> {
    let serving = Serving::start(routes())?;
    let body = "x".repeat(MAX_BODY);

    let answer = serving.ask(&format!(
        "POST /size HTTP/1.1\r\nContent-Length: {MAX_BODY}\r\n\r\n{body}"
    ))?;

    assert_eq!(status(&answer), "HTTP/1.1 200 OK");
    assert_eq!(split(&answer).1, MAX_BODY.to_string());
    Ok(())
}

// --- staying up ---

#[test]
fn a_panicking_handler_costs_one_connection_and_no_more() -> Result<(), Error> {
    let serving = Serving::start(routes())?;

    let broken = serving.ask("GET /boom HTTP/1.1\r\n\r\n")?;
    let next = serving.ask("GET /hello?name=ada HTTP/1.1\r\n\r\n")?;

    assert_eq!(status(&broken), "HTTP/1.1 500 Internal Server Error");
    assert_eq!(status(&next), "HTTP/1.1 200 OK");
    Ok(())
}

#[test]
fn serve_returns_when_the_flag_is_set() -> Result<(), Error> {
    let serving = Serving::start(routes())?;
    serving.ask("GET /hello?name=ada HTTP/1.1\r\n\r\n")?;

    assert!(serving.stop(), "serve did not return in {SHUTDOWN:?}");
    Ok(())
}

// --- events ---

#[test]
fn an_event_stream_opens_with_its_own_headers() -> Result<(), Error> {
    let serving = Serving::start(routes())?;

    let answer = serving.ask("GET /events HTTP/1.1\r\n\r\n")?;

    assert_eq!(
        split(&answer).0,
        "HTTP/1.1 200 OK\r\n\
         Content-Type: text/event-stream\r\n\
         Cache-Control: no-cache\r\n\
         Connection: keep-alive"
    );
    Ok(())
}

#[test]
fn the_events_arrive_one_frame_at_a_time() -> Result<(), Error> {
    let serving = Serving::start(routes())?;

    let answer = serving.ask("GET /events HTTP/1.1\r\n\r\n")?;

    assert_eq!(
        split(&answer).1,
        ": open\n\n\
         event: bar\ndata: passing\n\n\
         event: page\ndata: <p>\ndata:   one\ndata: </p>\n\n"
    );
    Ok(())
}
