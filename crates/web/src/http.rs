//! The server: a listener, a thread per connection, one request each.
//!
//! It binds where it is told, which is loopback, because this is a tool you
//! run in your own checkout and there is nobody else on the other end.
//!
//! The accept loop is non-blocking and looks at the shutdown flag every
//! [`ACCEPT_POLL`], so `serve` returns promptly when somebody hits ctrl-c
//! instead of waiting for one more connection to arrive.

use std::fmt;
use std::io::{self, BufReader};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::panic::{self, AssertUnwindSafe};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use crate::error::Error;
use crate::request::{Method, Rejected, Request};
use crate::response::{Body, Response};
use crate::sse::Sink;

/// How long the accept loop waits before looking at the shutdown flag again.
const ACCEPT_POLL: Duration = Duration::from_millis(50);

/// How long one connection may take to send its request line, headers and body.
const READ_TIMEOUT: Duration = Duration::from_secs(10);

/// What the server calls for every request it reads.
///
/// Shared across connection threads, so a handler holds what it needs by
/// value and locks whatever it shares.
pub type Handler = Arc<dyn Fn(Request) -> Reply + Send + Sync>;

/// What a handler gives back.
pub enum Reply {
    /// One response, written whole, and the connection closes.
    Once(Response),
    /// An event stream: the server sends the stream headers and hands the
    /// socket to the closure, which returns when it is done or the client left.
    Stream(Box<dyn FnOnce(&mut Sink) + Send>),
}

impl Reply {
    /// An event stream fed by that closure.
    #[must_use]
    pub fn stream(feed: impl FnOnce(&mut Sink) + Send + 'static) -> Self {
        Self::Stream(Box::new(feed))
    }
}

impl From<Response> for Reply {
    fn from(response: Response) -> Self {
        Self::Once(response)
    }
}

impl fmt::Debug for Reply {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Once(response) => f.debug_tuple("Once").field(response).finish(),
            Self::Stream(_) => f.write_str("Stream(..)"),
        }
    }
}

/// A bound listener waiting to be served.
#[derive(Debug)]
pub struct Server {
    listener: TcpListener,
    addr: SocketAddr,
}

impl Server {
    /// Bind an address, for example `127.0.0.1:0` to be given a free port.
    ///
    /// # Errors
    ///
    /// Fails when the address is taken, is not ours to bind, or does not
    /// resolve.
    pub fn bind(addr: &str) -> Result<Self, Error> {
        let listener = TcpListener::bind(addr).map_err(|cause| Error::Bind {
            addr: addr.to_owned(),
            cause,
        })?;
        let bound = listener.local_addr().map_err(|cause| Error::Bind {
            addr: addr.to_owned(),
            cause,
        })?;

        Ok(Self {
            listener,
            addr: bound,
        })
    }

    /// Where it actually bound, which is how you learn the port after `:0`.
    #[must_use]
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Answer requests until `shutdown` is set.
    ///
    /// Returns within [`ACCEPT_POLL`] of the flag flipping. Connections
    /// already open are not interrupted: a handler holding an event stream
    /// open watches the same flag and returns on it, which is what lets the
    /// process exit rather than wait for a browser tab.
    ///
    /// A handler that panics takes its own connection down with a 500 and
    /// nothing else. `catch_unwind` is not a design here, it is the promise
    /// that one bad route cannot end the review session.
    ///
    /// # Errors
    ///
    /// Fails when the listener itself breaks. A request the server cannot
    /// read is answered, not returned.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "the caller hands the server its own handles and then moves on; \
                  borrowing would make every caller keep them alive by hand"
    )]
    pub fn serve(&self, handler: Handler, shutdown: Arc<AtomicBool>) -> Result<(), Error> {
        self.listener.set_nonblocking(true)?;

        while !shutdown.load(Ordering::Relaxed) {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    let handler = Arc::clone(&handler);
                    thread::spawn(move || answer(&stream, handler.as_ref()));
                }
                Err(cause) if cause.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(ACCEPT_POLL);
                }
                Err(cause) if cause.kind() == io::ErrorKind::Interrupted => {}
                Err(cause) => return Err(Error::Io(cause)),
            }
        }

        Ok(())
    }
}

/// Read one request off the connection, answer it, and close.
fn answer(stream: &TcpStream, handler: &(dyn Fn(Request) -> Reply + Send + Sync)) {
    // A listener in non-blocking mode hands its accepted sockets the same flag
    // on macOS, where a read would then fail instead of waiting.
    let _ignored = stream.set_nonblocking(false);
    let _ignored = stream.set_read_timeout(Some(READ_TIMEOUT));
    let _ignored = stream.set_nodelay(true);

    let read = Request::parse(&mut BufReader::new(stream));
    let mut out = stream;

    let request = match read {
        Ok(request) => request,
        Err(Rejected::Gone) => return,
        Err(Rejected::Malformed) => {
            let _ignored = Response::plain(400, "malformed request").write_to(&mut out, Body::Send);
            return;
        }
        Err(Rejected::TooLarge) => {
            let _ignored = Response::plain(413, "body too large").write_to(&mut out, Body::Send);
            return;
        }
    };

    let body = match request.method() {
        Method::Head => Body::Omit,
        Method::Get | Method::Post | Method::Other(_) => Body::Send,
    };

    match panic::catch_unwind(AssertUnwindSafe(|| handler(request))) {
        Ok(Reply::Once(response)) => {
            let _ignored = response.write_to(&mut out, body);
        }
        Ok(Reply::Stream(feed)) => {
            if let Ok(mut sink) = Sink::open(stream) {
                let _ignored = panic::catch_unwind(AssertUnwindSafe(|| feed(&mut sink)));
            }
        }
        Err(_) => {
            let _ignored = Response::plain(500, "handler failed").write_to(&mut out, body);
        }
    }
}
