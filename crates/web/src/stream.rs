//! The two event streams, which are the only way a page is updated.
//!
//! Both are the same loop: render, send what moved, then park on the watcher
//! until something does. Nothing is sent when the client's fingerprint already
//! matches what the repository says, so a tab that reconnects is not handed a
//! page it is already showing. That single comparison is what the old surface
//! did not do, and it is why it pushed five megabytes of html at every load.

use std::io;
use std::sync::Arc;

use review::ProposalId;
use serde_json::{Value, json};

use crate::http::Reply;
use crate::model::Page;
use crate::render;
use crate::request::Request;
use crate::response::Response;
use crate::routes;
use crate::sse::Sink;
use crate::surface::Surface;
use crate::watch::Wakeup;

/// The stream behind one review page.
pub(crate) fn review(surface: &Arc<Surface>, id: &str, request: &Request) -> Reply {
    let Ok(id) = ProposalId::parse(id) else {
        return Response::not_found().into();
    };
    let since = routes::since(request);
    let known = request.query_value("fp").unwrap_or_default().to_owned();
    let head = request
        .query_value("head")
        .and_then(|raw| raw.parse::<u32>().ok());
    let surface = Arc::clone(surface);

    Reply::stream(move |sink| follow(&surface, &id, since, head, known, sink))
}

/// The stream behind the board.
pub(crate) fn board(surface: &Arc<Surface>, request: &Request) -> Reply {
    let known = request.query_value("fp").unwrap_or_default().to_owned();
    let surface = Arc::clone(surface);

    Reply::stream(move |sink| listing(&surface, known, sink))
}

/// Follow one proposal until the client leaves or the server stops.
fn follow(
    surface: &Arc<Surface>,
    id: &ProposalId,
    since: Option<u32>,
    head: Option<u32>,
    mut known: String,
    sink: &mut Sink,
) {
    let mut subscription = surface.subscribe();

    loop {
        if surface.stopped() {
            return;
        }
        match push(surface, id, since, head, &known, sink) {
            Ok(Some(fingerprint)) => known = fingerprint,
            Ok(None) => {}
            Err(_gone) => return,
        }
        match subscription.wait(surface.quiet()) {
            Wakeup::Changed => {}
            Wakeup::Timeout => {
                if sink.comment("ping").is_err() {
                    return;
                }
            }
            Wakeup::Stopped => return,
        }
    }
}

/// Send whatever moved since the client's fingerprint, and say what it is now.
///
/// A head that is not the one the page was rendered against is a new revision:
/// the diff itself changed, and no fragment can carry that, so the client is
/// told to fetch the page again.
fn push(
    surface: &Arc<Surface>,
    id: &ProposalId,
    since: Option<u32>,
    head: Option<u32>,
    known: &str,
    sink: &mut Sink,
) -> io::Result<Option<String>> {
    let page = match routes::built(surface, id, since) {
        Ok(Some(page)) => page,
        // A proposal that is not there any more, or a read that failed: the
        // stream stays open and says nothing rather than closing on a blip.
        Ok(None) => return Ok(None),
        Err(err) => {
            surface.log(&format!("stream {id}: {err}"));
            return Ok(None);
        }
    };

    if page.fingerprint() == known {
        return Ok(None);
    }

    if head.is_some_and(|shown| shown != page.head()) {
        sink.event("revision", &page.head().to_string())?;
    } else {
        sink.event("update", &update(&page))?;
    }

    Ok(Some(page.fingerprint().to_owned()))
}

/// What one update carries: the two swappable regions, and every thread that
/// exists now. The diff is never in here.
fn update(page: &Page) -> String {
    let threads: Vec<Value> = page
        .inline_threads()
        .iter()
        .map(|thread| {
            json!({
                "id": format!("t-{}", thread.id()),
                "after": thread.row(),
                "html": render::thread_row(thread).into_string(),
            })
        })
        .collect();

    json!({
        "fp": page.fingerprint(),
        "bar": render::bar(page).into_string(),
        "rail": render::rail(page).into_string(),
        "threads": threads,
        // The client removes any thread row the list above does not carry, so
        // this stays for the protocol and is always empty.
        "removed": Vec::<String>::new(),
    })
    .to_string()
}

/// Follow the whole board until the client leaves or the server stops.
fn listing(surface: &Arc<Surface>, mut known: String, sink: &mut Sink) {
    let mut subscription = surface.subscribe();

    loop {
        if surface.stopped() {
            return;
        }
        match routes::assemble(surface) {
            Ok(board) if board.fingerprint() != known => {
                if sink
                    .event("board", &render::board(&board).into_string())
                    .is_err()
                {
                    return;
                }
                board.fingerprint().clone_into(&mut known);
            }
            Ok(_unchanged) => {}
            Err(err) => surface.log(&format!("stream board: {err}")),
        }
        match subscription.wait(surface.quiet()) {
            Wakeup::Changed => {}
            Wakeup::Timeout => {
                if sink.comment("ping").is_err() {
                    return;
                }
            }
            Wakeup::Stopped => return,
        }
    }
}
