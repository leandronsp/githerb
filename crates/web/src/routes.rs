//! The route table, and the handler behind each route.
//!
//! Every route is one line of a match on the path segments; the method check
//! is the only thing wrapped around it, so a request for a route that exists
//! with a verb it does not have is told that rather than told nothing.
//!
//! A handler reads through the surface's caches, renders, and answers. The
//! ones that write call a use case in `app`, poke the watcher so every open
//! page moves at once, and answer with no content: what happened arrives on
//! the event stream, which is the only path the page is updated by.

use std::collections::HashMap;
use std::sync::Arc;

use review::{Anchor, FilePath, ProposalId, RecordId, Side, Span};

use crate::assets::static_asset;
use crate::http::Reply;
use crate::model::{Board, Page};
use crate::render;
use crate::request::{Method, Request};
use crate::response::Response;
use crate::stream;
use crate::surface::Surface;

/// A year, which is how long an asset named after its own contents is good
/// for.
const FOREVER: &str = "public, max-age=31536000, immutable";

/// Answer one request.
pub(crate) fn route(surface: &Arc<Surface>, request: &Request) -> Reply {
    let path = request.path().trim_matches('/').to_owned();
    let segments: Vec<&str> = path.split('/').collect();

    match segments.as_slice() {
        [""] => reading(request, || board(surface)),
        ["events"] => streaming(request, || stream::board(surface, request)),
        ["static", asset] => reading(request, || served(asset)),
        ["p", id] => reading(request, || page(surface, id, request)),
        ["p", id, "events"] => streaming(request, || stream::review(surface, id, request)),
        ["p", id, "file", index] => reading(request, || file(surface, id, index, request)),
        ["p", id, "handover"] => reading(request, || handover(surface, id)),
        ["p", id, "comments"] => writing(request, || comment(surface, id, request)),
        ["p", id, "replies"] => writing(request, || answer(surface, id, request)),
        ["p", id, "resolve"] => writing(request, || resolve(surface, id, request)),
        ["p", id, "dispatch"] => writing(request, || dispatch(surface, id)),
        ["p", id, "land"] => writing(request, || land(surface, id)),
        ["p", id, "abandon"] => writing(request, || abandon(surface, id)),
        _ => Response::not_found().into(),
    }
}

// --- reading ---

/// The board, with the sizes of every proposal on it.
fn board(surface: &Arc<Surface>) -> Reply {
    match assemble(surface) {
        Ok(board) => Response::html(render::board_page(&board).into_string()).into(),
        Err(err) => surface.refuse(&err).into(),
    }
}

/// One review page.
fn page(surface: &Arc<Surface>, id: &str, request: &Request) -> Reply {
    let Some(id) = named(id) else {
        return nothing().into();
    };

    match built(surface, &id, since(request)) {
        Ok(Some(page)) => Response::html(render::page(&page).into_string()).into(),
        Ok(None) => nothing().into(),
        Err(err) => surface.refuse(&err).into(),
    }
}

/// The table of one file, which is what a collapsed file fetches.
fn file(surface: &Arc<Surface>, id: &str, index: &str, request: &Request) -> Reply {
    let (Some(id), Ok(index)) = (named(id), index.parse::<usize>()) else {
        return Response::not_found().into();
    };

    match built(surface, &id, since(request)) {
        Ok(Some(page)) => render::file_table(&page, index)
            .map_or_else(Response::not_found, |table| {
                Response::html(table.into_string())
            })
            .into(),
        Ok(None) => Response::not_found().into(),
        Err(err) => surface.refuse(&err).into(),
    }
}

/// The brief a person hands to an agent.
fn handover(surface: &Arc<Surface>, id: &str) -> Reply {
    let Some(id) = named(id) else {
        return Response::not_found().into();
    };

    match app::handover(surface.store(), &id, app::Reader::Person) {
        Ok(text) => Response::text(text).into(),
        Err(err) => surface.refuse(&err).into(),
    }
}

/// The stylesheet or the script, cached for a year because the url carries a
/// hash of what it answers with.
fn served(asset: &str) -> Reply {
    match static_asset(&format!("/static/{asset}")) {
        Some((content_type, body)) => Response::asset(content_type, body)
            .with_header("Cache-Control", FOREVER)
            .into(),
        None => Response::not_found().into(),
    }
}

// --- writing ---

/// Leave a note on a span of the diff.
fn comment(surface: &Arc<Surface>, id: &str, request: &Request) -> Response {
    let Some(id) = named(id) else {
        return missing(id);
    };
    let form = request.form();
    let anchor = match anchored(&form) {
        Ok(anchor) => anchor,
        Err(err) => return Response::bad_request(err.to_string()),
    };

    surface.wrote(app::annotate(
        surface.store(),
        surface.author(),
        app::now(),
        &id,
        anchor,
        &field(&form, "body"),
    ))
}

/// Say something under a note.
fn answer(surface: &Arc<Surface>, id: &str, request: &Request) -> Response {
    let form = request.form();
    let (Some(id), Ok(note)) = (named(id), RecordId::parse(&field(&form, "note"))) else {
        return Response::bad_request("that is not a note this proposal carries");
    };

    surface.wrote(app::reply(
        surface.store(),
        surface.author(),
        app::now(),
        &id,
        &note,
        &field(&form, "body"),
    ))
}

/// Say a note is answered.
fn resolve(surface: &Arc<Surface>, id: &str, request: &Request) -> Response {
    let form = request.form();
    let (Some(id), Ok(note)) = (named(id), RecordId::parse(&field(&form, "note"))) else {
        return Response::bad_request("that is not a note this proposal carries");
    };

    surface.wrote(app::resolve(
        surface.store(),
        surface.author(),
        app::now(),
        &id,
        &note,
    ))
}

/// Hand the open notes to an agent.
fn dispatch(surface: &Arc<Surface>, id: &str) -> Response {
    let Some(id) = named(id) else {
        return missing(id);
    };

    surface.wrote(app::dispatch(
        surface.store(),
        surface.author(),
        app::now(),
        &id,
    ))
}

/// Move the target branch onto the head, if the gate lets it.
fn land(surface: &Arc<Surface>, id: &str) -> Response {
    let Some(id) = named(id) else {
        return missing(id);
    };
    let required = match surface.required() {
        Ok(required) => required,
        Err(err) => return surface.refuse(&err),
    };

    surface.wrote(app::land(
        surface.store(),
        &required,
        surface.author(),
        app::now(),
        &id,
    ))
}

/// Give up on a proposal.
fn abandon(surface: &Arc<Surface>, id: &str) -> Response {
    let Some(id) = named(id) else {
        return missing(id);
    };

    surface.wrote(app::abandon(
        surface.store(),
        surface.author(),
        app::now(),
        &id,
    ))
}

// --- what the handlers share ---

/// The page model of a proposal, or nothing when there is no such proposal.
pub(crate) fn built(
    surface: &Arc<Surface>,
    id: &ProposalId,
    since: Option<u32>,
) -> app::Result<Option<Page>> {
    let Some(proposal) = surface.proposal(id)? else {
        return Ok(None);
    };
    let patch = surface.patch(&proposal, since)?;
    let required = surface.required()?;

    Ok(Some(Page::build(&proposal, &patch, &required, since)))
}

/// The board, with `(added, removed)` per proposal taken from the diff cache.
///
/// A proposal whose diff git will not produce is still listed; it shows no
/// counts rather than taking the whole board down with it.
pub(crate) fn assemble(surface: &Arc<Surface>) -> app::Result<Board> {
    let snapshot = surface.snapshot()?;
    let mut sizes = HashMap::new();

    for proposal in snapshot.proposals() {
        match surface.patch(proposal, None) {
            Ok(patch) => {
                sizes.insert(proposal.id().clone(), (patch.added(), patch.removed()));
            }
            Err(err) => surface.log(&format!("sizing {}: {err}", proposal.id())),
        }
    }

    Ok(Board::build(snapshot.proposals(), &sizes))
}

/// Which revision the reader is measuring from. Anything that is not a number
/// is the base, which is what an old link with a stale query should show.
pub(crate) fn since(request: &Request) -> Option<u32> {
    request
        .query_value("since")
        .and_then(|raw| raw.parse::<u32>().ok())
        .filter(|number| *number > 0)
}

/// The page that says there is no proposal by that name.
pub(crate) fn nothing() -> Response {
    Response::missing(render::missing("No proposal is called that.").into_string())
}

/// A name that could be a proposal's.
fn named(id: &str) -> Option<ProposalId> {
    ProposalId::parse(id).ok()
}

/// A refusal naming a proposal that does not exist.
fn missing(id: &str) -> Response {
    Response::plain(404, &format!("proposal {id}: not found"))
}

/// The span a note was left on, as the form spelled it.
fn anchored(form: &[(String, String)]) -> review::Result<Anchor> {
    let file = FilePath::parse(&field(form, "file"))?;
    let side = Side::parse(&field(form, "side"))?;
    let span = Span::new(side, number(form, "start"), number(form, "end"))?;

    Ok(Anchor::new(file, span))
}

/// One field of a form, empty when the form did not carry it.
fn field(form: &[(String, String)], name: &str) -> String {
    form.iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.clone())
        .unwrap_or_default()
}

/// One numeric field. Zero stands for anything unreadable, and the constructor
/// refuses zero, so a broken form is refused by the domain rather than here.
fn number(form: &[(String, String)], name: &str) -> u32 {
    field(form, name).parse().unwrap_or(0)
}

/// Whether the request may read.
fn reading(request: &Request, answer: impl FnOnce() -> Reply) -> Reply {
    match request.method() {
        Method::Get | Method::Head => answer(),
        Method::Post | Method::Other(_) => Response::method_not_allowed().into(),
    }
}

/// Whether the request may open a stream. A HEAD is not allowed to: there is
/// no end to the headers of a stream, so answering one would hang a client
/// that only asked what is there.
fn streaming(request: &Request, answer: impl FnOnce() -> Reply) -> Reply {
    match request.method() {
        Method::Get => answer(),
        Method::Head | Method::Post | Method::Other(_) => Response::method_not_allowed().into(),
    }
}

/// Whether the request may write.
fn writing(request: &Request, answer: impl FnOnce() -> Response) -> Reply {
    match request.method() {
        Method::Post => answer().into(),
        Method::Get | Method::Head | Method::Other(_) => Response::method_not_allowed().into(),
    }
}
