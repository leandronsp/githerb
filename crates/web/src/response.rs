//! One HTTP response and the bytes it becomes.
//!
//! Every response closes the connection. This is a tool you run in your own
//! checkout, the browser opens one long-lived event stream and short requests
//! around it, so keep-alive would buy nothing and cost a state machine.

use std::io::{self, Write};

/// Whether the body follows the headers. A HEAD asks for the headers alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Body {
    /// Write the body after the headers.
    Send,
    /// Stop after the headers, but count the body in `Content-Length`.
    Omit,
}

/// What the server answers with.
///
/// `Content-Length` and `Connection` are written from the body and never from
/// the header list, so a handler cannot make the framing disagree with itself.
#[derive(Debug, Clone)]
pub struct Response {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Response {
    /// A page.
    #[must_use]
    pub fn html(body: impl Into<String>) -> Self {
        Self::of(200, "text/html; charset=utf-8", body.into().into_bytes())
    }

    /// A line of text.
    #[must_use]
    pub fn text(body: impl Into<String>) -> Self {
        Self::of(200, "text/plain; charset=utf-8", body.into().into_bytes())
    }

    /// A JSON document, already serialised.
    #[must_use]
    pub fn json(body: impl Into<String>) -> Self {
        Self::of(200, "application/json", body.into().into_bytes())
    }

    /// Done, and there is nothing to say.
    #[must_use]
    pub fn no_content() -> Self {
        Self {
            status: 204,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    /// Go and read that instead, with a GET whatever this request was.
    #[must_use]
    pub fn redirect(location: impl Into<String>) -> Self {
        Self {
            status: 303,
            headers: vec![("Location".to_owned(), location.into())],
            body: Vec::new(),
        }
    }

    /// No such route.
    #[must_use]
    pub fn not_found() -> Self {
        Self::plain(404, "not found")
    }

    /// A file served as itself, whose type the caller already knows.
    #[must_use]
    pub fn asset(content_type: &str, body: impl Into<String>) -> Self {
        Self::of(200, content_type, body.into().into_bytes())
    }

    /// The page that says there is nothing at that address.
    #[must_use]
    pub fn missing(body: impl Into<String>) -> Self {
        Self::of(404, "text/html; charset=utf-8", body.into().into_bytes())
    }

    /// The request was understood and refused; the message says why.
    #[must_use]
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::plain(400, &message.into())
    }

    /// That route exists, but not for that verb.
    #[must_use]
    pub fn method_not_allowed() -> Self {
        Self::plain(405, "method not allowed")
    }

    /// Add a header. Repeating a name sends the header twice, as HTTP allows.
    #[must_use]
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// The status code.
    #[must_use]
    pub fn status(&self) -> u16 {
        self.status
    }

    /// The headers set so far, without the ones the server writes itself.
    #[must_use]
    pub fn headers(&self) -> &[(String, String)] {
        &self.headers
    }

    /// The body bytes.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// A bare status with a one-line explanation.
    pub(crate) fn plain(status: u16, message: &str) -> Self {
        Self::of(
            status,
            "text/plain; charset=utf-8",
            message.as_bytes().to_vec(),
        )
    }

    fn of(status: u16, content_type: &str, body: Vec<u8>) -> Self {
        Self {
            status,
            headers: vec![("Content-Type".to_owned(), content_type.to_owned())],
            body,
        }
    }

    /// Write the whole response, headers and all, and flush it.
    pub(crate) fn write_to<W: Write>(&self, out: &mut W, body: Body) -> io::Result<()> {
        let mut head = String::with_capacity(128);
        head.push_str("HTTP/1.1 ");
        head.push_str(&self.status.to_string());
        head.push(' ');
        head.push_str(reason(self.status));
        head.push_str("\r\n");

        for (name, value) in &self.headers {
            if framing(name) {
                continue;
            }
            head.push_str(name);
            head.push_str(": ");
            head.push_str(value);
            head.push_str("\r\n");
        }

        // A 204 says "nothing follows" by its status, and a Content-Length on
        // it is the one framing header HTTP forbids.
        if self.status != 204 {
            head.push_str("Content-Length: ");
            head.push_str(&self.body.len().to_string());
            head.push_str("\r\n");
        }
        head.push_str("Connection: close\r\n\r\n");

        out.write_all(head.as_bytes())?;
        match body {
            Body::Send => out.write_all(&self.body)?,
            Body::Omit => {}
        }

        out.flush()
    }
}

/// Whether the server writes that header itself.
fn framing(name: &str) -> bool {
    name.eq_ignore_ascii_case("content-length") || name.eq_ignore_ascii_case("connection")
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        204 => "No Content",
        303 => "See Other",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Content Too Large",
        500 => "Internal Server Error",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written(response: &Response, body: Body) -> String {
        let mut out: Vec<u8> = Vec::new();
        response.write_to(&mut out, body).unwrap();
        String::from_utf8(out).unwrap()
    }

    // --- the wire ---

    #[test]
    fn writes_a_status_line_headers_and_the_body() {
        let response = Response::text("hello");

        assert_eq!(
            written(&response, Body::Send),
            "HTTP/1.1 200 OK\r\n\
             Content-Type: text/plain; charset=utf-8\r\n\
             Content-Length: 5\r\n\
             Connection: close\r\n\
             \r\n\
             hello"
        );
    }

    #[test]
    fn a_head_keeps_the_length_and_drops_the_body() {
        let response = Response::html("<p>hi</p>");

        assert_eq!(
            written(&response, Body::Omit),
            "HTTP/1.1 200 OK\r\n\
             Content-Type: text/html; charset=utf-8\r\n\
             Content-Length: 9\r\n\
             Connection: close\r\n\
             \r\n"
        );
    }

    #[test]
    fn a_handler_cannot_write_the_framing_headers() {
        let response = Response::text("hi")
            .with_header("Content-Length", "999")
            .with_header("Connection", "keep-alive")
            .with_header("X-Note", "kept");

        assert_eq!(
            written(&response, Body::Send),
            "HTTP/1.1 200 OK\r\n\
             Content-Type: text/plain; charset=utf-8\r\n\
             X-Note: kept\r\n\
             Content-Length: 2\r\n\
             Connection: close\r\n\
             \r\n\
             hi"
        );
    }

    #[test]
    fn no_content_carries_no_length() {
        assert_eq!(
            written(&Response::no_content(), Body::Send),
            "HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n"
        );
    }

    #[test]
    fn a_redirect_says_where_to() {
        assert_eq!(
            written(&Response::redirect("/p/one"), Body::Send),
            "HTTP/1.1 303 See Other\r\n\
             Location: /p/one\r\n\
             Content-Length: 0\r\n\
             Connection: close\r\n\
             \r\n"
        );
    }

    #[test]
    fn the_refusals_carry_their_status() {
        assert_eq!(Response::not_found().status(), 404);
        assert_eq!(Response::bad_request("no such file").status(), 400);
        assert_eq!(Response::method_not_allowed().status(), 405);
        assert_eq!(
            Response::bad_request("no such file").body(),
            b"no such file"
        );
    }
}
