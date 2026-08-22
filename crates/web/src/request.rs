//! One HTTP request, read off the socket and nothing more.
//!
//! The parser is deliberately small: a request line, headers until the blank
//! line, then exactly `Content-Length` bytes. It refuses what it cannot read
//! rather than guessing, and it refuses a body over a megabyte, because this
//! server answers a review page on loopback and nothing else. Every ceiling
//! here exists so one confused client cannot make a thread hold memory or a
//! socket forever.

use std::fmt;
use std::io::{BufRead, Read};

use crate::form::{Encoding, decode, pairs, pick};

/// The largest body the server will read, in bytes.
pub const MAX_BODY: usize = 1024 * 1024;

/// The largest a request line or a single header line may be, in bytes.
const MAX_LINE: u64 = 8 * 1024;

/// The largest number of header lines a request may carry.
const MAX_HEADERS: usize = 100;

/// What a request asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Method {
    /// Read something.
    Get,
    /// Write something.
    Post,
    /// Read what a `Get` would have sent, headers only.
    Head,
    /// Any other verb, kept verbatim so a handler can answer 405 by name.
    Other(String),
}

impl Method {
    /// Read a verb off the request line. An unknown one is kept as `Other`.
    #[must_use]
    pub fn parse(raw: &str) -> Self {
        match raw {
            "GET" => Self::Get,
            "POST" => Self::Post,
            "HEAD" => Self::Head,
            other => Self::Other(other.to_owned()),
        }
    }

    /// The verb as it travels on the wire.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Head => "HEAD",
            Self::Other(verb) => verb,
        }
    }
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why a request never became one.
///
/// The server turns each of these into an answer and closes: nothing here
/// stops it serving the next connection.
#[derive(Debug)]
pub(crate) enum Rejected {
    /// The connection ended or failed before a whole request arrived, so
    /// there is nobody left to answer.
    Gone,
    /// The request line or a header did not parse, or ran past its ceiling.
    Malformed,
    /// The declared body is larger than [`MAX_BODY`].
    TooLarge,
}

/// One request, parsed.
///
/// Path and query arrive percent-decoded. The query keeps the order the
/// client sent and may repeat a name, which is why it is a list and not a map.
#[derive(Debug, Clone)]
pub struct Request {
    method: Method,
    path: String,
    query: Vec<(String, String)>,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Request {
    /// The verb.
    #[must_use]
    pub fn method(&self) -> &Method {
        &self.method
    }

    /// The path, percent-decoded, without the query string.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The query pairs, decoded, in the order they arrived.
    #[must_use]
    pub fn query(&self) -> &[(String, String)] {
        &self.query
    }

    /// The headers, in the order they arrived, names as the client cased them.
    #[must_use]
    pub fn headers(&self) -> &[(String, String)] {
        &self.headers
    }

    /// The body bytes, exactly as many as `Content-Length` promised.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// The first value the query gives that name.
    #[must_use]
    pub fn query_value(&self, name: &str) -> Option<&str> {
        pick(&self.query, name)
    }

    /// The first value of that header, matched without regard to case.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(field, _)| field.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// The body read as an HTML form.
    ///
    /// `application/x-www-form-urlencoded`: pairs split on `&`, name and value
    /// split on the first `=`, `+` read as a space, `%xx` decoded, and the
    /// result read as UTF-8 with anything invalid replaced. The content type
    /// is not consulted, so a `fetch` that forgets to set it still works.
    #[must_use]
    pub fn form(&self) -> Vec<(String, String)> {
        pairs(&String::from_utf8_lossy(&self.body))
    }

    /// The first value the form gives that name.
    ///
    /// This decodes the body on every call, which is the right trade for a
    /// handler reading two fields; read [`Request::form`] once for more.
    #[must_use]
    pub fn form_value(&self, name: &str) -> Option<String> {
        self.form()
            .into_iter()
            .find(|(field, _)| field == name)
            .map(|(_, value)| value)
    }

    /// Read one request off a buffered connection.
    pub(crate) fn parse<R: BufRead>(reader: &mut R) -> Result<Self, Rejected> {
        let line = read_line(reader)?;
        let mut words = line.split_whitespace();
        let (Some(verb), Some(target)) = (words.next(), words.next()) else {
            return Err(Rejected::Malformed);
        };
        let (path, query) = split_target(target);

        let mut headers: Vec<(String, String)> = Vec::new();
        loop {
            let line = read_line(reader)?;
            if line.is_empty() {
                break;
            }
            if headers.len() >= MAX_HEADERS {
                return Err(Rejected::Malformed);
            }
            let Some((name, value)) = line.split_once(':') else {
                return Err(Rejected::Malformed);
            };
            headers.push((name.trim().to_owned(), value.trim().to_owned()));
        }

        let mut body = vec![0_u8; content_length(&headers)?];
        reader
            .read_exact(&mut body)
            .map_err(|_ended| Rejected::Gone)?;

        Ok(Self {
            method: Method::parse(verb),
            path,
            query,
            headers,
            body,
        })
    }
}

fn read_line<R: BufRead>(reader: &mut R) -> Result<String, Rejected> {
    let mut line = String::new();
    let read = reader
        .by_ref()
        .take(MAX_LINE)
        .read_line(&mut line)
        .map_err(|_ended| Rejected::Gone)?;

    if read == 0 {
        return Err(Rejected::Gone);
    }
    // Nothing ended the line inside the ceiling, so the line is over it.
    if !line.ends_with('\n') {
        return Err(Rejected::Malformed);
    }

    Ok(line.trim_end_matches(['\r', '\n']).to_owned())
}

fn content_length(headers: &[(String, String)]) -> Result<usize, Rejected> {
    let Some((_, raw)) = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
    else {
        return Ok(0);
    };

    let length: usize = raw.parse().map_err(|_ignored| Rejected::Malformed)?;
    if length > MAX_BODY {
        return Err(Rejected::TooLarge);
    }

    Ok(length)
}

fn split_target(target: &str) -> (String, Vec<(String, String)>) {
    match target.split_once('?') {
        Some((path, query)) => (decode(path, Encoding::Path), pairs(query)),
        None => (decode(target, Encoding::Path), Vec::new()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufReader;

    fn parse(raw: &str) -> Result<Request, Rejected> {
        Request::parse(&mut BufReader::new(raw.as_bytes()))
    }

    fn refusal(raw: &str) -> &'static str {
        match parse(raw) {
            Ok(_) => "parsed",
            Err(Rejected::Gone) => "gone",
            Err(Rejected::Malformed) => "malformed",
            Err(Rejected::TooLarge) => "too large",
        }
    }

    // --- the request line ---

    #[test]
    fn reads_the_verb_and_the_path() -> Result<(), Rejected> {
        let request = parse("GET /p/one HTTP/1.1\r\n\r\n")?;

        assert_eq!(request.method(), &Method::Get);
        assert_eq!(request.path(), "/p/one");
        Ok(())
    }

    #[test]
    fn keeps_a_verb_it_does_not_know() -> Result<(), Rejected> {
        let request = parse("PATCH / HTTP/1.1\r\n\r\n")?;

        assert_eq!(request.method(), &Method::Other("PATCH".to_owned()));
        assert_eq!(request.method().as_str(), "PATCH");
        Ok(())
    }

    #[test]
    fn decodes_the_path_but_leaves_a_plus_alone() -> Result<(), Rejected> {
        let request = parse("GET /p/a%2Fb+c HTTP/1.1\r\n\r\n")?;

        assert_eq!(request.path(), "/p/a/b+c");
        Ok(())
    }

    #[test]
    fn splits_the_query_off_the_path() -> Result<(), Rejected> {
        let request = parse("GET /r?file=a.rs&side=new&start= HTTP/1.1\r\n\r\n")?;

        assert_eq!(request.path(), "/r");
        assert_eq!(request.query_value("file"), Some("a.rs"));
        assert_eq!(request.query_value("side"), Some("new"));
        assert_eq!(request.query_value("start"), Some(""));
        assert_eq!(request.query_value("end"), None);
        Ok(())
    }

    #[test]
    fn refuses_a_request_line_that_says_nothing() {
        assert_eq!(refusal("GET\r\n\r\n"), "malformed");
    }

    #[test]
    fn refuses_a_header_line_over_the_ceiling() {
        let long = "x".repeat(usize::try_from(MAX_LINE).unwrap() + 10);

        assert_eq!(
            refusal(&format!("GET / HTTP/1.1\r\nX: {long}\r\n\r\n")),
            "malformed"
        );
    }

    // --- headers ---

    #[test]
    fn finds_a_header_whatever_its_case() -> Result<(), Rejected> {
        let request = parse("GET / HTTP/1.1\r\nAccept: text/html\r\nX-Note:  spaced \r\n\r\n")?;

        assert_eq!(request.header("accept"), Some("text/html"));
        assert_eq!(request.header("X-NOTE"), Some("spaced"));
        assert_eq!(request.header("missing"), None);
        assert_eq!(request.headers().len(), 2);
        Ok(())
    }

    // --- the body ---

    #[test]
    fn reads_exactly_content_length_bytes() -> Result<(), Rejected> {
        let request = parse("POST /c HTTP/1.1\r\nContent-Length: 5\r\n\r\nhelloTRAILING")?;

        assert_eq!(request.body(), b"hello");
        Ok(())
    }

    #[test]
    fn a_body_over_the_ceiling_is_too_large() {
        let head = format!(
            "POST /c HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            MAX_BODY + 1
        );

        assert_eq!(refusal(&head), "too large");
    }

    #[test]
    fn a_body_at_the_ceiling_is_read() -> Result<(), Rejected> {
        let body = "x".repeat(MAX_BODY);
        let request = parse(&format!(
            "POST /c HTTP/1.1\r\nContent-Length: {MAX_BODY}\r\n\r\n{body}"
        ))?;

        assert_eq!(request.body().len(), MAX_BODY);
        Ok(())
    }

    #[test]
    fn decodes_a_form_body() -> Result<(), Rejected> {
        let body = "body=one+two&file=a%2Fb.rs&note=%C3%A9&flag";
        let request = parse(&format!(
            "POST /c HTTP/1.1\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        ))?;

        assert_eq!(
            request.form(),
            vec![
                ("body".to_owned(), "one two".to_owned()),
                ("file".to_owned(), "a/b.rs".to_owned()),
                ("note".to_owned(), "é".to_owned()),
                ("flag".to_owned(), String::new()),
            ]
        );
        assert_eq!(request.form_value("body"), Some("one two".to_owned()));
        assert_eq!(request.form_value("nothing"), None);
        Ok(())
    }
}
