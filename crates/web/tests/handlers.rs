//! The routes against a real repository.
//!
//! A fake git proves nothing about git, so every test here builds a real
//! repository in a temp directory, opens a proposal through the use cases a
//! person would use, and then speaks raw HTTP to the server the binary
//! serves. Nothing sleeps to synchronise: a write pokes the watcher, and the
//! stream's own read timeout is what bounds a test that is waiting for
//! nothing.

// A test binary; the crate root allows the same three under `cfg(test)`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use std::{env, thread};

use app::Store;
use gitstore::Repo;
use review::{Author, ProposalId, Timestamp};
use web::{Server, Surface};

/// How long a client waits for an answer before calling the server broken.
const PATIENCE: Duration = Duration::from_secs(5);

/// How often the watcher probes, and how long a quiet stream waits before it
/// says something. Short, because a test should not sit through fifteen
/// seconds to see one heartbeat.
const TICK: Duration = Duration::from_millis(50);

static COUNTER: AtomicU64 = AtomicU64::new(0);

// --- the repository ---

/// A repository built for one test, removed when the test ends.
struct TempRepo {
    home: PathBuf,
    root: PathBuf,
}

impl TempRepo {
    fn new() -> Self {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let home = env::temp_dir().join(format!("githerb-web-{}-{id}", std::process::id()));
        fs::create_dir_all(&home).unwrap();

        // macOS hands out a symlinked temp dir and git answers with the real
        // path, so the comparison only works from the resolved one.
        let home = home.canonicalize().unwrap();
        let root = home.join("repo");
        fs::create_dir_all(&root).unwrap();

        let repo = Self { home, root };
        repo.git(&["init", "-q", "-b", "main"]);
        repo.git(&["config", "user.name", "test"]);
        repo.git(&["config", "user.email", "test@githerb"]);
        repo.write(
            "README.md",
            "# githerb\n\nA gate for trunk.\n\nNo server.\n",
        );
        repo.git(&["add", "README.md"]);
        repo.git(&["commit", "-q", "-m", "root"]);

        repo
    }

    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );

        String::from_utf8(output.stdout)
            .unwrap()
            .trim_end()
            .to_owned()
    }

    fn write(&self, path: &str, text: &str) {
        fs::write(self.root.join(path), text).unwrap();
    }

    fn store(&self) -> Store {
        Store::new(Repo::open(&self.root).unwrap())
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ignored = fs::remove_dir_all(&self.home);
    }
}

// --- the server ---

/// A surface serving one repository on a port of its own.
struct Serving {
    temp: TempRepo,
    store: Store,
    id: ProposalId,
    addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
}

impl Serving {
    /// A repository with one proposal open on a side branch, and a server in
    /// front of it.
    fn start() -> Self {
        let temp = TempRepo::new();
        temp.git(&["checkout", "-q", "-B", "gate"]);
        temp.write(
            "README.md",
            "# githerb (demo)\n\nA gate for trunk.\n\nNo server.\n",
        );
        temp.git(&["add", "README.md"]);
        temp.git(&["commit", "-q", "-m", "the work"]);

        let store = temp.store();
        let proposal =
            app::propose(&store, &author(), now(), "Demo slice", "main", "HEAD").unwrap();
        let id = proposal.id().clone();

        let watcher = web::watching(&store, TICK);
        let surface = Surface::new(store.clone(), author(), watcher, Box::new(|_ignored| {}));
        surface.heartbeat(TICK);

        let server = Server::bind("127.0.0.1:0").unwrap();
        let addr = server.addr();
        let shutdown = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&shutdown);
        thread::spawn(move || web::serve(&surface, &server, flag));

        Self {
            temp,
            store,
            id,
            addr,
            shutdown,
        }
    }

    /// Send one raw request and read the answer to end of connection.
    fn ask(&self, raw: &str) -> String {
        let mut socket = TcpStream::connect(self.addr).unwrap();
        socket.set_read_timeout(Some(PATIENCE)).unwrap();
        socket.write_all(raw.as_bytes()).unwrap();
        socket.flush().unwrap();

        let mut answer = Vec::new();
        socket.read_to_end(&mut answer).unwrap();

        String::from_utf8_lossy(&answer).into_owned()
    }

    fn get(&self, path: &str) -> String {
        self.ask(&format!("GET {path} HTTP/1.1\r\nHost: x\r\n\r\n"))
    }

    fn post(&self, path: &str, form: &str) -> String {
        self.ask(&format!(
            "POST {path} HTTP/1.1\r\nHost: x\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n\r\n{form}",
            form.len()
        ))
    }

    /// The review page of the proposal this server was started with.
    fn page(&self) -> String {
        self.get(&format!("/p/{}", self.id))
    }

    /// The fingerprint the page was rendered at, which is what a stream sends.
    fn fingerprint(&self) -> String {
        let page = self.page();
        let (_, rest) = page.split_once("data-fp=\"").unwrap();
        let (fingerprint, _) = rest.split_once('"').unwrap();

        fingerprint.to_owned()
    }
}

impl Drop for Serving {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

fn author() -> Author {
    Author::parse("leandro").unwrap()
}

fn now() -> Timestamp {
    Timestamp::from_unix(1_787_000_645)
}

/// The status line of an answer.
fn status(answer: &str) -> &str {
    answer.lines().next().unwrap_or_default()
}

/// The body of an answer.
fn body(answer: &str) -> &str {
    answer.split_once("\r\n\r\n").map_or("", |(_, body)| body)
}

// --- reading ---

#[test]
fn the_board_lists_what_is_proposed() {
    let serving = Serving::start();
    let answer = serving.get("/");

    assert_eq!(status(&answer), "HTTP/1.1 200 OK");
    assert!(body(&answer).contains("Demo slice"), "{}", body(&answer));
    assert!(body(&answer).contains("in review"), "{}", body(&answer));
}

#[test]
fn a_review_page_carries_the_files_and_a_row_per_line() {
    let serving = Serving::start();
    let answer = serving.page();
    let html = body(&answer);

    assert_eq!(status(&answer), "HTTP/1.1 200 OK");
    assert!(
        html.contains(r#"<section class="file" id="f-0" data-path="README.md">"#),
        "{html}"
    );
    assert!(html.contains(r#"<tr id="L-0-0""#), "{html}");
    assert!(html.contains("# githerb (demo)"), "{html}");
}

#[test]
fn no_proposal_by_that_name_is_a_page_that_says_so() {
    let serving = Serving::start();
    let answer = serving.get("/p/nobody-here-0000000");

    assert_eq!(status(&answer), "HTTP/1.1 404 Not Found");
    assert!(
        body(&answer).contains("No proposal is called that."),
        "{}",
        body(&answer)
    );
}

#[test]
fn a_route_that_does_not_exist_is_not_found() {
    let serving = Serving::start();

    assert_eq!(status(&serving.get("/nowhere")), "HTTP/1.1 404 Not Found");
}

#[test]
fn a_write_route_asked_to_read_says_which_verb_it_wants() {
    let serving = Serving::start();
    let answer = serving.get(&format!("/p/{}/comments", serving.id));

    assert_eq!(status(&answer), "HTTP/1.1 405 Method Not Allowed");
}

#[test]
fn the_assets_are_served_with_a_year_of_cache() {
    let serving = Serving::start();

    for asset in ["review.css", "review.js"] {
        let answer = serving.get(&format!("/static/{asset}?v=whatever"));

        assert_eq!(status(&answer), "HTTP/1.1 200 OK");
        assert!(
            answer.contains("Cache-Control: public, max-age=31536000, immutable"),
            "{asset}: {answer}"
        );
    }
}

// --- writing ---

#[test]
fn a_note_comes_back_as_a_thread_under_the_line_it_is_on() {
    let serving = Serving::start();
    let answer = serving.post(
        &format!("/p/{}/comments", serving.id),
        "file=README.md&side=new&start=1&end=1&body=drop+the+suffix",
    );
    assert_eq!(status(&answer), "HTTP/1.1 204 No Content");

    let html = serving.page();
    assert!(
        html.contains(r#"</tr><tr class="thread-row" id="t-"#),
        "the thread is not in the diff: {html}"
    );
    assert!(html.contains(r#"data-after="L-0-1""#), "{html}");
    assert!(html.contains("drop the suffix"), "{html}");
}

#[test]
fn a_span_that_ends_before_it_starts_is_refused_in_plain_words() {
    let serving = Serving::start();
    let answer = serving.post(
        &format!("/p/{}/comments", serving.id),
        "file=README.md&side=new&start=3&end=2&body=nope",
    );

    assert_eq!(status(&answer), "HTTP/1.1 400 Bad Request");
    assert!(!body(&answer).contains("git "), "{}", body(&answer));
    assert!(body(&answer).contains("3 to 2"), "{}", body(&answer));
}

#[test]
fn a_side_that_is_not_a_side_is_refused() {
    let serving = Serving::start();
    let answer = serving.post(
        &format!("/p/{}/comments", serving.id),
        "file=README.md&side=sideways&start=1&end=1&body=nope",
    );

    assert_eq!(status(&answer), "HTTP/1.1 400 Bad Request");
    assert!(body(&answer).contains("sideways"), "{}", body(&answer));
}

#[test]
fn an_answer_joins_the_thread_and_a_resolution_ends_it() {
    let serving = Serving::start();
    let comment = app::annotate(
        &serving.store,
        &author(),
        now(),
        &serving.id,
        anchor(),
        "drop the suffix",
    )
    .unwrap();

    let answer = serving.post(
        &format!("/p/{}/replies", serving.id),
        &format!("note={}&body=done+in+r2", comment.id()),
    );
    assert_eq!(status(&answer), "HTTP/1.1 204 No Content");
    assert!(serving.page().contains("done in r2"));

    let answer = serving.post(
        &format!("/p/{}/resolve", serving.id),
        &format!("note={}", comment.id()),
    );
    assert_eq!(status(&answer), "HTTP/1.1 204 No Content");

    let html = serving.page();
    assert!(!html.contains("thread-row"), "the thread is still inline");
    assert!(html.contains("resolved (1)"), "{html}");
}

#[test]
fn a_note_that_this_proposal_never_carried_is_refused() {
    let serving = Serving::start();
    let answer = serving.post(&format!("/p/{}/resolve", serving.id), "note=000000000000");

    assert_eq!(status(&answer), "HTTP/1.1 400 Bad Request");
}

#[test]
fn landing_is_refused_while_a_note_is_open_and_taken_once_it_is_not() {
    let serving = Serving::start();
    let comment = app::annotate(
        &serving.store,
        &author(),
        now(),
        &serving.id,
        anchor(),
        "not yet",
    )
    .unwrap();

    let answer = serving.post(&format!("/p/{}/land", serving.id), "");
    assert_eq!(status(&answer), "HTTP/1.1 400 Bad Request");
    assert!(body(&answer).contains("open comments"), "{}", body(&answer));

    app::resolve(&serving.store, &author(), now(), &serving.id, comment.id()).unwrap();
    let answer = serving.post(&format!("/p/{}/land", serving.id), "");
    assert_eq!(status(&answer), "HTTP/1.1 204 No Content");
    assert_eq!(
        serving.temp.git(&["rev-parse", "refs/heads/main"]),
        serving.temp.git(&["rev-parse", "refs/heads/gate"])
    );
}

#[test]
fn the_handover_is_text_and_carries_the_note() {
    let serving = Serving::start();
    app::annotate(
        &serving.store,
        &author(),
        now(),
        &serving.id,
        anchor(),
        "drop the suffix",
    )
    .unwrap();

    let answer = serving.get(&format!("/p/{}/handover", serving.id));

    assert_eq!(status(&answer), "HTTP/1.1 200 OK");
    assert!(answer.contains("Content-Type: text/plain"), "{answer}");
    assert!(
        body(&answer).contains("drop the suffix"),
        "{}",
        body(&answer)
    );
}

/// The one line of the README the tests annotate.
fn anchor() -> review::Anchor {
    review::Anchor::new(
        review::FilePath::parse("README.md").unwrap(),
        review::Span::new(review::Side::New, 1, 1).unwrap(),
    )
}

// --- the stream ---

/// One open event stream, read incrementally.
struct Listening {
    socket: TcpStream,
    seen: String,
}

impl Listening {
    fn open(serving: &Serving, path: &str) -> Self {
        let mut socket = TcpStream::connect(serving.addr).unwrap();
        socket.set_read_timeout(Some(TICK * 8)).unwrap();
        socket
            .write_all(format!("GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").as_bytes())
            .unwrap();
        socket.flush().unwrap();

        Self {
            socket,
            seen: String::new(),
        }
    }

    /// Read until the marker arrives, or give up after `patience`.
    fn until(&mut self, marker: &str, patience: Duration) -> bool {
        let deadline = Instant::now() + patience;

        while !self.seen.contains(marker) {
            if Instant::now() > deadline {
                return false;
            }
            let mut buffer = [0_u8; 4096];
            match self.socket.read(&mut buffer) {
                Ok(0) => return false,
                Ok(read) => self
                    .seen
                    .push_str(&String::from_utf8_lossy(&buffer[..read])),
                Err(_timeout) => {}
            }
        }

        true
    }

    /// Whether the server closed the connection.
    fn ended(&mut self) -> bool {
        let deadline = Instant::now() + PATIENCE;

        loop {
            if Instant::now() > deadline {
                return false;
            }
            let mut buffer = [0_u8; 4096];
            match self.socket.read(&mut buffer) {
                Ok(0) => return true,
                Ok(read) => self
                    .seen
                    .push_str(&String::from_utf8_lossy(&buffer[..read])),
                Err(_timeout) => {}
            }
        }
    }
}

#[test]
fn a_stream_that_is_already_current_is_sent_nothing_but_a_heartbeat() {
    let serving = Serving::start();
    let mut stream = Listening::open(
        &serving,
        &format!(
            "/p/{}/events?fp={}&head=1",
            serving.id,
            serving.fingerprint()
        ),
    );

    assert!(
        stream.until(": ping", PATIENCE),
        "no heartbeat: {}",
        stream.seen
    );
    assert!(
        !stream.seen.contains("event: update"),
        "pushed a page the client already has: {}",
        stream.seen
    );
}

#[test]
fn a_note_left_anywhere_arrives_on_the_stream_as_a_thread() {
    let serving = Serving::start();
    let mut stream = Listening::open(
        &serving,
        &format!(
            "/p/{}/events?fp={}&head=1",
            serving.id,
            serving.fingerprint()
        ),
    );
    assert!(stream.until("HTTP/1.1 200", PATIENCE), "{}", stream.seen);

    serving.post(
        &format!("/p/{}/comments", serving.id),
        "file=README.md&side=new&start=1&end=1&body=drop+the+suffix",
    );

    assert!(
        stream.until("event: update", PATIENCE),
        "nothing arrived: {}",
        stream.seen
    );
    assert!(stream.seen.contains("thread-row"), "{}", stream.seen);
    assert!(stream.seen.contains("drop the suffix"), "{}", stream.seen);
    assert!(
        stream.seen.contains(r#"data-after=\"L-0-1\""#),
        "the thread is not anchored: {}",
        stream.seen
    );
}

#[test]
fn a_stream_ends_when_the_server_does() {
    let serving = Serving::start();
    let mut stream = Listening::open(
        &serving,
        &format!(
            "/p/{}/events?fp={}&head=1",
            serving.id,
            serving.fingerprint()
        ),
    );
    assert!(stream.until("HTTP/1.1 200", PATIENCE), "{}", stream.seen);

    serving.shutdown.store(true, Ordering::Relaxed);

    assert!(
        stream.ended(),
        "the stream outlived the server: {}",
        stream.seen
    );
}

#[test]
fn a_new_revision_tells_the_page_to_fetch_itself_again() {
    let serving = Serving::start();
    let mut stream = Listening::open(
        &serving,
        &format!(
            "/p/{}/events?fp={}&head=1",
            serving.id,
            serving.fingerprint()
        ),
    );
    assert!(stream.until("HTTP/1.1 200", PATIENCE), "{}", stream.seen);

    serving.temp.write(
        "README.md",
        "# githerb\n\nA gate for trunk.\n\nNo server.\n",
    );
    serving.temp.git(&["add", "README.md"]);
    serving.temp.git(&["commit", "-q", "-m", "the fix"]);
    app::revise(&serving.store, &serving.id, "HEAD").unwrap();

    assert!(
        stream.until("event: revision", PATIENCE),
        "the page was never told: {}",
        stream.seen
    );
    assert!(stream.seen.contains("data: 2"), "{}", stream.seen);
}

#[test]
fn the_board_stream_pushes_the_listing_when_a_proposal_moves() {
    let serving = Serving::start();
    let board = serving.get("/");
    let (_, rest) = board.split_once("data-fp=\"").unwrap();
    let (fingerprint, _) = rest.split_once('"').unwrap();

    let mut stream = Listening::open(&serving, &format!("/events?fp={fingerprint}"));
    assert!(stream.until("HTTP/1.1 200", PATIENCE), "{}", stream.seen);

    serving.post(
        &format!("/p/{}/comments", serving.id),
        "file=README.md&side=new&start=1&end=1&body=one+note",
    );

    assert!(
        stream.until("event: board", PATIENCE),
        "the board never moved: {}",
        stream.seen
    );
    assert!(stream.seen.contains("1 note"), "{}", stream.seen);
}
