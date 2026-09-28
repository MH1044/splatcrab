//! The UI server's HTTP layer: request bytes in, response bytes out.
//!
//! [`handle`] is a pure function of the bytes of one request, the session and
//! the [`Config`], so every status and header is pinned by unit tests and by
//! the `.http` golden cases without a socket. `src/server.rs` puts a loopback
//! socket around it and [`serve_stdio`] puts stdin and stdout around it; both
//! frame requests with [`read_request`], which enforces the size limits
//! before a byte past them is buffered.
//!
//! The routes, statuses, header order and checks are fixed by
//! `docs/modules/U1-ui-server.md`:
//!
//! | Route | Method | Answer |
//! |---|---|---|
//! | `/` | `GET` | the page, with its `Content-Security-Policy` |
//! | `/app.js`, `/app.css` | `GET` | the page's script and stylesheet |
//! | `/api` | `POST` | [`protocol::respond`] to the body, less its newline |
//!
//! Every request must name this server in `Host`, and in `Origin` when it
//! sends one; `/api` also needs the session token in `X-SplatCrab-Token` and
//! a JSON content type. A request that fails a check is answered with an
//! error status before the interpreter is touched: only the last line of
//! [`handle`] reaches it.

use std::io::{self, BufRead, Read, Write};

use crate::error;
use crate::interp::Interp;
use crate::protocol;

/// The request line and headers together, blank line included, may be at
/// most this many bytes; one more is `431`.
pub const MAX_HEAD: usize = 16 * 1024;

/// A body may be at most this many bytes; a `Content-Length` of one more is
/// `413`, answered without reading the body.
pub const MAX_BODY: usize = 8 * 1024 * 1024;

/// The page, its script and its stylesheet, embedded in the binary.
pub const PAGE: &str = include_str!("ui/index.html");
pub const SCRIPT: &str = include_str!("ui/app.js");
pub const STYLE: &str = include_str!("ui/app.css");

/// Sent with the page only: no inline script or style, nothing from another
/// origin, and no framing by another page.
pub const CSP: &str = "default-src 'self'; frame-ancestors 'none'";

/// The header that carries the session token on every `/api` call.
pub const TOKEN_HEADER: &str = "X-SplatCrab-Token";

const HTML: &str = "text/html; charset=utf-8";
const JS: &str = "text/javascript; charset=utf-8";
const CSS: &str = "text/css; charset=utf-8";
const JSON: &str = "application/json";
const TEXT: &str = "text/plain; charset=utf-8";

/// What a request is checked against: the port the server is bound to, which
/// `Host` and `Origin` must name, and the session token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub port: u16,
    pub token: String,
}

/// Every status this server answers with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Ok,
    BadRequest,
    Forbidden,
    NotFound,
    MethodNotAllowed,
    ContentTooLarge,
    UnsupportedMediaType,
    HeaderFieldsTooLarge,
    NotImplemented,
}

impl Status {
    pub fn code(self) -> u16 {
        match self {
            Status::Ok => 200,
            Status::BadRequest => 400,
            Status::Forbidden => 403,
            Status::NotFound => 404,
            Status::MethodNotAllowed => 405,
            Status::ContentTooLarge => 413,
            Status::UnsupportedMediaType => 415,
            Status::HeaderFieldsTooLarge => 431,
            Status::NotImplemented => 501,
        }
    }

    /// `403 Forbidden`: the status line after `HTTP/1.1 `, and the body of an
    /// error response. The texts are in `error.rs`.
    pub fn text(self) -> &'static str {
        error::http_status(self.code())
    }
}

/// The request line and headers of one request, borrowed from its bytes.
struct Head<'a> {
    method: &'a [u8],
    /// The target up to any `?`: the query plays no part in routing.
    path: &'a [u8],
    headers: Vec<(&'a [u8], &'a [u8])>,
    /// Checked against [`MAX_BODY`] already.
    content_length: usize,
    /// Bytes from the start of the request line through the blank line.
    len: usize,
}

impl<'a> Head<'a> {
    /// Every value of the header `name`, matched case-insensitively.
    fn values(&self, name: &str) -> Vec<&'a [u8]> {
        self.headers
            .iter()
            .filter(|(n, _)| n.eq_ignore_ascii_case(name.as_bytes()))
            .map(|(_, v)| *v)
            .collect()
    }

    /// The value of a header that must appear at most once: `Ok(None)` when
    /// it is absent, `Err` when it appears more than once.
    fn single(&self, name: &str) -> Result<Option<&'a [u8]>, ()> {
        match self.values(name)[..] {
            [] => Ok(None),
            [v] => Ok(Some(v)),
            _ => Err(()),
        }
    }
}

/// The offset just past the empty line that ends the head, or `None` when
/// the bytes hold no empty line. A line ends at LF, with or without a CR
/// before it.
fn head_end(bytes: &[u8]) -> Option<usize> {
    let mut start = 0;
    while let Some(k) = bytes[start..].iter().position(|&b| b == b'\n') {
        let line = &bytes[start..start + k];
        if line.is_empty() || line == b"\r" {
            return Some(start + k + 1);
        }
        start += k + 1;
    }
    None
}

/// RFC 9110's `tchar`: what a method or a header name is made of.
fn is_tchar(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b)
}

fn is_token(s: &[u8]) -> bool {
    !s.is_empty() && s.iter().all(|&b| is_tchar(b))
}

fn trim_ows(mut s: &[u8]) -> &[u8] {
    while let [b' ' | b'\t', rest @ ..] = s {
        s = rest;
    }
    while let [rest @ .., b' ' | b'\t'] = s {
        s = rest;
    }
    s
}

/// Parses and checks the head: framing, the request line, header syntax,
/// `Transfer-Encoding` and `Content-Length`. What it refuses is refused for
/// every route alike, before any check of who is asking. It checks only
/// what the body's framing depends on, so that a head it accepts always has
/// its body read and a stream of requests stays in step; [`handle`] checks
/// the rest.
fn parse_head(bytes: &[u8]) -> Result<Head<'_>, Status> {
    let len = match head_end(bytes) {
        Some(end) if end <= MAX_HEAD => end,
        Some(_) => return Err(Status::HeaderFieldsTooLarge),
        None if bytes.len() > MAX_HEAD => return Err(Status::HeaderFieldsTooLarge),
        None => return Err(Status::BadRequest),
    };
    let mut lines = bytes[..len].split(|&b| b == b'\n').map(|l| match l {
        [rest @ .., b'\r'] => rest,
        _ => l,
    });

    let request_line = lines.next().unwrap_or_default();
    let (method, target) = match request_line.split(|&b| b == b' ').collect::<Vec<_>>()[..] {
        [m, t, v] if is_token(m) && (v == b"HTTP/1.1" || v == b"HTTP/1.0") => (m, t),
        _ => return Err(Status::BadRequest),
    };
    if target.first() != Some(&b'/') || !target.iter().all(|&b| (0x21..=0x7e).contains(&b)) {
        return Err(Status::BadRequest);
    }
    let path = match target.iter().position(|&b| b == b'?') {
        Some(q) => &target[..q],
        None => target,
    };

    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            break;
        }
        // A line that starts with whitespace is an obsolete line folding,
        // which RFC 9112 lets a server refuse.
        let colon = line
            .iter()
            .position(|&b| b == b':')
            .ok_or(Status::BadRequest)?;
        let (name, value) = (&line[..colon], trim_ows(&line[colon + 1..]));
        let bad_byte = |&b: &u8| (b < 0x20 && b != b'\t') || b == 0x7f;
        if !is_token(name) || value.iter().any(bad_byte) {
            return Err(Status::BadRequest);
        }
        headers.push((name, value));
    }

    let mut head = Head {
        method,
        path,
        headers,
        content_length: 0,
        len,
    };
    // No transfer coding is supported, so the body could not be framed.
    if !head.values("Transfer-Encoding").is_empty() {
        return Err(Status::NotImplemented);
    }
    if let Some(v) = head
        .single("Content-Length")
        .map_err(|()| Status::BadRequest)?
    {
        if !is_digits(v) {
            return Err(Status::BadRequest);
        }
        // Saturating, so a length too long for any integer is still just
        // too large rather than malformed.
        let n = v.iter().fold(0u64, |n, &d| {
            n.saturating_mul(10).saturating_add(u64::from(d - b'0'))
        });
        if n > MAX_BODY as u64 {
            return Err(Status::ContentTooLarge);
        }
        head.content_length = n as usize;
    }
    Ok(head)
}

fn is_digits(s: &[u8]) -> bool {
    !s.is_empty() && s.iter().all(u8::is_ascii_digit)
}

/// Compares in time that depends on the lengths only, so that how long a
/// refusal takes says nothing about how much of a guessed token was right.
fn same_secret(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// `Host` names this server: `127.0.0.1:<port>` or `localhost:<port>`. A
/// name any other page could resolve to the loopback, as DNS rebinding does,
/// is refused, and so is a missing `Host`.
fn host_ok(head: &Head, cfg: &Config) -> bool {
    let Ok(Some(host)) = head.single("Host") else {
        return false;
    };
    [
        format!("127.0.0.1:{}", cfg.port),
        format!("localhost:{}", cfg.port),
    ]
    .iter()
    .any(|want| host.eq_ignore_ascii_case(want.as_bytes()))
}

/// An `Origin`, when a browser sends one, is this server's own. No `Origin`
/// at all is allowed: a same-origin `GET` has none.
fn origin_ok(head: &Head, cfg: &Config) -> bool {
    match head.single("Origin") {
        Ok(None) => true,
        Ok(Some(origin)) => [
            format!("http://127.0.0.1:{}", cfg.port),
            format!("http://localhost:{}", cfg.port),
        ]
        .iter()
        .any(|want| origin.eq_ignore_ascii_case(want.as_bytes())),
        Err(()) => false,
    }
}

/// Exactly one token header, equal to the session's token.
fn token_ok(head: &Head, cfg: &Config) -> bool {
    matches!(head.single(TOKEN_HEADER), Ok(Some(t)) if same_secret(t, cfg.token.as_bytes()))
}

/// `application/json`, in any case, with or without parameters.
fn json_body(head: &Head) -> bool {
    let Ok(Some(ct)) = head.single("Content-Type") else {
        return false;
    };
    let media = ct.split(|&b| b == b';').next().unwrap_or_default();
    trim_ows(media).eq_ignore_ascii_case(JSON.as_bytes())
}

/// One response: the status line, the headers in the spec's order, a blank
/// line and the body. `Content-Length` is the body's byte count by
/// construction.
fn response(
    status: Status,
    content_type: &str,
    body: &[u8],
    csp: bool,
    allow: Option<&str>,
) -> Vec<u8> {
    let mut head = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n\
         Referrer-Policy: no-referrer\r\n",
        status.text(),
        content_type,
        body.len()
    );
    if csp {
        head.push_str(&format!("Content-Security-Policy: {}\r\n", CSP));
    }
    if let Some(methods) = allow {
        head.push_str(&format!("Allow: {}\r\n", methods));
    }
    head.push_str("Connection: close\r\n\r\n");
    let mut out = head.into_bytes();
    out.extend_from_slice(body);
    out
}

/// An error status, with its own text as a plain-text body.
fn refuse(status: Status) -> Vec<u8> {
    response(status, TEXT, status.text().as_bytes(), false, None)
}

/// `405`, naming the one method the path takes.
fn wrong_method(allow: &str) -> Vec<u8> {
    let status = Status::MethodNotAllowed;
    response(status, TEXT, status.text().as_bytes(), false, Some(allow))
}

/// The response to one request.
///
/// `request` is the whole request: the head, then at least `Content-Length`
/// bytes of body. Bytes after the body are not part of it and are ignored.
/// The interpreter is reached only by a well-formed `POST /api` that passed
/// every check.
pub fn handle(request: &[u8], it: &mut Interp, cfg: &Config) -> Vec<u8> {
    let head = match parse_head(request) {
        Ok(head) => head,
        Err(status) => return refuse(status),
    };
    // RFC 9112 asks for 400 when a request carries more than one Host.
    if head.single("Host").is_err() {
        return refuse(Status::BadRequest);
    }
    let Some(body) = request[head.len..].get(..head.content_length) else {
        // The input ended before the body did.
        return refuse(Status::BadRequest);
    };
    if !host_ok(&head, cfg) || !origin_ok(&head, cfg) {
        return refuse(Status::Forbidden);
    }
    let get = head.method == b"GET";
    let file = |content_type: &str, text: &str, csp: bool| {
        response(Status::Ok, content_type, text.as_bytes(), csp, None)
    };
    match head.path {
        b"/" if get => file(HTML, PAGE, true),
        b"/app.js" if get => file(JS, SCRIPT, false),
        b"/app.css" if get => file(CSS, STYLE, false),
        b"/" | b"/app.js" | b"/app.css" => wrong_method("GET"),
        b"/api" if head.method != b"POST" => wrong_method("POST"),
        b"/api" if !token_ok(&head, cfg) => refuse(Status::Forbidden),
        b"/api" if !json_body(&head) => refuse(Status::UnsupportedMediaType),
        b"/api" => {
            // Decoded leniently, as `--protocol` decodes a line: invalid
            // UTF-8 becomes U+FFFD and, outside a string, a malformed
            // request, which is an answer.
            let text = String::from_utf8_lossy(body);
            let reply = protocol::respond(it, &text).to_string();
            response(Status::Ok, JSON, reply.as_bytes(), false, None)
        }
        _ => refuse(Status::NotFound),
    }
}

/// One request as framed off a stream, ready for [`handle`].
#[derive(Debug, PartialEq, Eq)]
pub struct Request {
    pub bytes: Vec<u8>,
    /// False when the head was cut off, by the size cap or by the end of
    /// the input, before its empty line was read. The rest of that head is
    /// still in the stream.
    pub head_complete: bool,
}

/// Reads one request from `input`: the head up to its empty line, and then,
/// when [`parse_head`] accepts that head, exactly `Content-Length` bytes of
/// body. `None` at end of input before the first byte of a request.
///
/// Both limits are judged before buffering. The head is read a line at a
/// time under a budget of `MAX_HEAD + 1` bytes, so a head that never ends
/// stops the read one byte past the cap, which [`handle`] answers `431`. A
/// body is read only once its length has been checked against `MAX_BODY`;
/// a head that is refused, `413` included, has its body left unread.
///
/// `skip_empty_lines` is the stdio rule: empty lines before a request line
/// are dropped rather than read as a request, since the LF that ends a
/// hand-written body is one. The socket does not skip them.
pub fn read_request(
    input: &mut impl BufRead,
    skip_empty_lines: bool,
) -> io::Result<Option<Request>> {
    let mut bytes = Vec::new();
    loop {
        let start = bytes.len();
        let room = (MAX_HEAD + 1 - start) as u64;
        if input.by_ref().take(room).read_until(b'\n', &mut bytes)? == 0 {
            return Ok((!bytes.is_empty()).then_some(Request {
                bytes,
                head_complete: false,
            }));
        }
        let line = &bytes[start..];
        if line == b"\n" || line == b"\r\n" {
            if start == 0 && skip_empty_lines {
                bytes.clear();
                continue;
            }
            break;
        }
        if bytes.len() > MAX_HEAD {
            return Ok(Some(Request {
                bytes,
                head_complete: false,
            }));
        }
    }
    if let Ok(head) = parse_head(&bytes) {
        let n = head.content_length as u64;
        input.by_ref().take(n).read_to_end(&mut bytes)?;
    }
    Ok(Some(Request {
        bytes,
        head_complete: true,
    }))
}

/// After a head cut off by the cap, reads and drops the rest of it, through
/// its empty line, without buffering it. `tail` is what was read of the head
/// after its last LF, the start of the line the cap cut through.
///
/// Its body, if it has one, is not read: its length is in a header nobody
/// parsed. The socket never needs this, since it closes the connection.
fn discard_head(input: &mut impl BufRead, tail: &[u8]) -> io::Result<()> {
    // The line in progress: its length so far, and whether it is `\r` alone.
    let (mut len, mut cr) = (tail.len(), tail == b"\r");
    loop {
        let buf = input.fill_buf()?;
        if buf.is_empty() {
            return Ok(());
        }
        let mut used = 0;
        for &b in buf {
            used += 1;
            if b == b'\n' {
                if len == 0 || (len == 1 && cr) {
                    input.consume(used);
                    return Ok(());
                }
                (len, cr) = (0, false);
            } else {
                len += 1;
                cr = len == 1 && b == b'\r';
            }
        }
        input.consume(used);
    }
}

/// `splatcrab --http-stdio`: [`handle`] with no socket, over a fresh session.
pub fn serve_stdio(input: impl BufRead, output: impl Write, cfg: &Config) -> io::Result<()> {
    let mut it = Interp::with_output(Box::new(io::sink()));
    serve_stdio_with(&mut it, input, output, cfg)
}

/// [`serve_stdio`] against an interpreter the caller owns.
///
/// Requests are read one after another until the input ends. Each response
/// is followed by one `\n`, a separator in the stream rather than part of
/// the response, so that a body ends its line. `Ok` at end of input; `Err`
/// only when reading or writing the stream itself fails.
pub fn serve_stdio_with(
    it: &mut Interp,
    mut input: impl BufRead,
    mut output: impl Write,
    cfg: &Config,
) -> io::Result<()> {
    while let Some(req) = read_request(&mut input, true)? {
        let reply = handle(&req.bytes, it, cfg);
        output.write_all(&reply)?;
        output.write_all(b"\n")?;
        output.flush()?;
        if !req.head_complete {
            let tail_at = req
                .bytes
                .iter()
                .rposition(|&b| b == b'\n')
                .map_or(0, |k| k + 1);
            discard_head(&mut input, &req.bytes[tail_at..])?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config {
        Config {
            port: 8123,
            token: "test-token".to_string(),
        }
    }

    fn fresh() -> Interp {
        Interp::with_output(Box::new(io::sink()))
    }

    /// A parsed response: status line, headers in order, body.
    struct Reply {
        status: String,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    }

    impl Reply {
        fn header(&self, name: &str) -> Option<&str> {
            self.headers
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v.as_str())
        }
        fn names(&self) -> Vec<&str> {
            self.headers.iter().map(|(n, _)| n.as_str()).collect()
        }
        fn text(&self) -> &str {
            std::str::from_utf8(&self.body).expect("UTF-8 body")
        }
    }

    /// Splits a response and checks what every response must hold: CRLF
    /// line ends, `Content-Length` equal to the body's length, no `Date` or
    /// `Server`, and `Connection: close` last.
    fn parse(raw: &[u8]) -> Reply {
        let split = raw
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .expect("a blank line ends the head");
        let head = std::str::from_utf8(&raw[..split]).expect("ASCII head");
        let body = raw[split + 4..].to_vec();
        let mut lines = head.split("\r\n");
        let status = lines.next().unwrap().to_string();
        assert!(status.starts_with("HTTP/1.1 "), "{status}");
        let headers: Vec<(String, String)> = lines
            .map(|l| {
                assert!(!l.contains('\n'), "a bare LF in {l:?}");
                let (n, v) = l.split_once(": ").expect("name: value");
                (n.to_string(), v.to_string())
            })
            .collect();
        let reply = Reply {
            status,
            headers,
            body,
        };
        assert_eq!(
            reply.header("Content-Length"),
            Some(reply.body.len().to_string().as_str()),
            "Content-Length must count the body"
        );
        assert_eq!(reply.names().last(), Some(&"Connection"));
        assert_eq!(reply.header("Connection"), Some("close"));
        assert!(reply.header("Date").is_none() && reply.header("Server").is_none());
        reply
    }

    fn send(it: &mut Interp, raw: &str) -> Reply {
        parse(&handle(raw.as_bytes(), it, &cfg()))
    }

    fn one(raw: &str) -> Reply {
        send(&mut fresh(), raw)
    }

    /// A `POST /api` with every check passing.
    fn api(body: &str) -> String {
        format!(
            "POST /api HTTP/1.1\r\nHost: 127.0.0.1:8123\r\nX-SplatCrab-Token: test-token\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        )
    }

    /// A `POST /api` of `x = 1` with the given headers, and nothing else.
    fn api_with(headers: &str) -> String {
        let body = r#"{"id":1,"op":"eval","code":"x = 1"}"#;
        format!(
            "POST /api HTTP/1.1\r\n{headers}Content-Length: {}\r\n\r\n{body}",
            body.len()
        )
    }

    const GOOD: &str = "Host: 127.0.0.1:8123\r\nX-SplatCrab-Token: test-token\r\n\
                        Content-Type: application/json\r\n";

    fn get(path: &str) -> String {
        format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:8123\r\n\r\n")
    }

    fn assert_refused(reply: &Reply, status: &str) {
        assert_eq!(reply.status, format!("HTTP/1.1 {status}"));
        assert_eq!(reply.text(), status);
        assert_eq!(reply.header("Content-Type"), Some(TEXT));
    }

    #[test]
    fn every_status_has_its_text_in_the_table() {
        use Status::*;
        let all = [
            Ok,
            BadRequest,
            Forbidden,
            NotFound,
            MethodNotAllowed,
            ContentTooLarge,
            UnsupportedMediaType,
            HeaderFieldsTooLarge,
            NotImplemented,
        ];
        for s in all {
            assert!(
                error::HTTP_STATUS.iter().any(|(c, _)| *c == s.code()),
                "{s:?}"
            );
            assert!(s.text().starts_with(&format!("{} ", s.code())));
        }
        assert_eq!(Forbidden.text(), "403 Forbidden");
        assert_eq!(ContentTooLarge.text(), "413 Content Too Large");
    }

    #[test]
    fn eval_through_api_answers_the_protocol_json_less_its_newline() {
        let r = one(&api(r#"{"id":1,"op":"eval","code":"x = 1 + 2"}"#));
        assert_eq!(r.status, "HTTP/1.1 200 OK");
        assert_eq!(r.text(), r#"{"id":1,"ok":true,"out":"x =\n\n     3\n\n"}"#);
        assert_eq!(r.header("Content-Length"), Some("44"));
        assert_eq!(r.header("Content-Type"), Some("application/json"));
    }

    #[test]
    fn headers_come_in_the_specified_order() {
        let base = [
            "Content-Type",
            "Content-Length",
            "Cache-Control",
            "X-Content-Type-Options",
            "Referrer-Policy",
        ];
        let with = |extra: &[&'static str]| {
            let mut v: Vec<&str> = base.to_vec();
            v.extend_from_slice(extra);
            v.push("Connection");
            v
        };
        let page = one(&get("/"));
        assert_eq!(page.names(), with(&["Content-Security-Policy"]));
        assert_eq!(page.header("Content-Security-Policy"), Some(CSP));
        assert_eq!(page.header("Cache-Control"), Some("no-store"));
        assert_eq!(page.header("X-Content-Type-Options"), Some("nosniff"));
        assert_eq!(page.header("Referrer-Policy"), Some("no-referrer"));
        assert_eq!(one(&get("/app.js")).names(), with(&[]));
        assert_eq!(one(&api("{}")).names(), with(&[]));
        assert_eq!(one(&get("/nope")).names(), with(&[]));
        assert_eq!(one(&get("/api")).names(), with(&["Allow"]));
    }

    #[test]
    fn the_status_line_and_headers_are_byte_exact() {
        let raw = handle(get("/nope").as_bytes(), &mut fresh(), &cfg());
        assert_eq!(
            String::from_utf8(raw).unwrap(),
            "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain; charset=utf-8\r\n\
             Content-Length: 13\r\nCache-Control: no-store\r\n\
             X-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\n\
             Connection: close\r\n\r\n404 Not Found"
        );
    }

    #[test]
    fn static_routes_serve_the_embedded_files() {
        for (path, ty, text) in [
            ("/", HTML, PAGE),
            ("/app.js", JS, SCRIPT),
            ("/app.css", CSS, STYLE),
        ] {
            let r = one(&get(path));
            assert_eq!(r.status, "HTTP/1.1 200 OK", "{path}");
            assert_eq!(r.header("Content-Type"), Some(ty), "{path}");
            assert_eq!(r.body, text.as_bytes(), "{path}");
        }
        // Only the page carries the policy.
        assert!(
            one(&get("/app.js"))
                .header("Content-Security-Policy")
                .is_none()
        );
        // A query string plays no part in routing.
        assert_eq!(one(&get("/?x=1")).body, PAGE.as_bytes());
    }

    #[test]
    fn the_page_loads_nothing_inline_or_from_elsewhere() {
        assert!(PAGE.contains(r#"src="/app.js""#) && PAGE.contains(r#"href="/app.css""#));
        assert!(!PAGE.contains("<style") && !PAGE.contains("style="));
        assert!(!PAGE.contains("<script>") && !PAGE.contains("onclick"));
        for file in [PAGE, SCRIPT, STYLE] {
            assert!(!file.contains("http://") && !file.contains("https://"));
        }
        assert!(SCRIPT.contains(TOKEN_HEADER));
    }

    #[test]
    fn not_found_and_method_not_allowed() {
        assert_refused(&one(&get("/nope")), "404 Not Found");
        let r = one(&get("/api"));
        assert_refused(&r, "405 Method Not Allowed");
        assert_eq!(r.header("Allow"), Some("POST"));
        let r = one("POST / HTTP/1.1\r\nHost: 127.0.0.1:8123\r\n\r\n");
        assert_refused(&r, "405 Method Not Allowed");
        assert_eq!(r.header("Allow"), Some("GET"));
        // A CORS preflight is never approved.
        let r = one("OPTIONS /api HTTP/1.1\r\nHost: 127.0.0.1:8123\r\n\r\n");
        assert_eq!(r.header("Allow"), Some("POST"));
    }

    #[test]
    fn the_token_is_required_and_must_match() {
        let no_token = "Host: 127.0.0.1:8123\r\nContent-Type: application/json\r\n";
        assert_refused(&one(&api_with(no_token)), "403 Forbidden");
        let wrong = format!("{no_token}X-SplatCrab-Token: wrong\r\n");
        assert_refused(&one(&api_with(&wrong)), "403 Forbidden");
        let prefix = format!("{no_token}X-SplatCrab-Token: test-toke\r\n");
        assert_refused(&one(&api_with(&prefix)), "403 Forbidden");
        let twice = format!("{GOOD}X-SplatCrab-Token: test-token\r\n");
        assert_refused(&one(&api_with(&twice)), "403 Forbidden");
        assert_eq!(one(&api_with(GOOD)).status, "HTTP/1.1 200 OK");
    }

    #[test]
    fn a_refused_request_never_reaches_the_interpreter() {
        let mut it = fresh();
        let refusals = [
            "Host: 127.0.0.1:8123\r\nContent-Type: application/json\r\n".to_string(),
            "Host: 127.0.0.1:8123\r\nX-SplatCrab-Token: wrong\r\nContent-Type: application/json\r\n"
                .to_string(),
            GOOD.replace("127.0.0.1:8123", "evil.example:8123"),
            format!("{GOOD}Origin: http://evil.example\r\n"),
            GOOD.replace("application/json", "text/plain"),
        ];
        for headers in &refusals {
            let r = send(&mut it, &api_with(headers));
            assert_ne!(r.status, "HTTP/1.1 200 OK", "{headers}");
            assert!(it.vars.is_empty(), "{headers}");
        }
        assert_eq!(send(&mut it, &api_with(GOOD)).status, "HTTP/1.1 200 OK");
        assert!(it.vars.contains_key("x"));
    }

    #[test]
    fn host_must_name_this_server() {
        for host in [
            "evil.example:8123",
            "127.0.0.1:9999",
            "127.0.0.1",
            "[::1]:8123",
        ] {
            let headers = GOOD.replace("127.0.0.1:8123", host);
            assert_refused(&one(&api_with(&headers)), "403 Forbidden");
        }
        for host in ["localhost:8123", "LOCALHOST:8123", "127.0.0.1:8123"] {
            let headers = GOOD.replace("127.0.0.1:8123", host);
            assert_eq!(one(&api_with(&headers)).status, "HTTP/1.1 200 OK", "{host}");
        }
        // No Host at all is refused, on a static route too.
        assert_refused(&one("GET / HTTP/1.1\r\n\r\n"), "403 Forbidden");
        assert_refused(
            &one("GET / HTTP/1.1\r\nHost: evil.example:8123\r\n\r\n"),
            "403 Forbidden",
        );
        // Two Host headers are malformed, as RFC 9112 says.
        let twice = format!("{GOOD}Host: 127.0.0.1:8123\r\n");
        assert_refused(&one(&api_with(&twice)), "400 Bad Request");
    }

    #[test]
    fn origin_when_present_must_be_this_server() {
        for origin in [
            "http://evil.example",
            "null",
            "http://127.0.0.1:9999",
            "https://127.0.0.1:8123",
        ] {
            let headers = format!("{GOOD}Origin: {origin}\r\n");
            assert_refused(&one(&api_with(&headers)), "403 Forbidden");
        }
        for origin in ["http://127.0.0.1:8123", "http://localhost:8123"] {
            let headers = format!("{GOOD}Origin: {origin}\r\n");
            assert_eq!(one(&api_with(&headers)).status, "HTTP/1.1 200 OK");
        }
    }

    #[test]
    fn api_needs_a_json_content_type() {
        let plain = GOOD.replace("application/json", "text/plain");
        assert_refused(&one(&api_with(&plain)), "415 Unsupported Media Type");
        let none = "Host: 127.0.0.1:8123\r\nX-SplatCrab-Token: test-token\r\n";
        assert_refused(&one(&api_with(none)), "415 Unsupported Media Type");
        let with_charset = GOOD.replace("application/json", "Application/JSON; charset=utf-8");
        assert_eq!(one(&api_with(&with_charset)).status, "HTTP/1.1 200 OK");
    }

    #[test]
    fn header_names_are_case_insensitive_and_lf_ends_a_line() {
        let r = one(&api_with(
            "host: 127.0.0.1:8123\r\nx-splatcrab-token: test-token\r\nCONTENT-TYPE: application/json\r\n",
        ));
        assert_eq!(r.status, "HTTP/1.1 200 OK");
        let lf = api(r#"{"id":1,"op":"eval","code":"disp(7)"}"#).replace("\r\n", "\n");
        let r = one(&lf);
        assert_eq!(r.text(), r#"{"id":1,"ok":true,"out":"     7\n"}"#);
        // Whitespace around a value is not part of it.
        let spaced = GOOD.replace(": test-token", ":  test-token \t");
        assert_eq!(one(&api_with(&spaced)).status, "HTTP/1.1 200 OK");
    }

    #[test]
    fn malformed_requests_are_bad_requests() {
        for raw in [
            "hello\r\n\r\n",
            "hello",
            "\r\n",
            "GET /\r\n\r\n",
            "GET / HTTP/2\r\nHost: 127.0.0.1:8123\r\n\r\n",
            "GET  / HTTP/1.1\r\nHost: 127.0.0.1:8123\r\n\r\n",
            "GET nope HTTP/1.1\r\nHost: 127.0.0.1:8123\r\n\r\n",
            "GET / HTTP/1.1\r\nHost 127.0.0.1:8123\r\n\r\n",
            "GET / HTTP/1.1\r\nHost : 127.0.0.1:8123\r\n\r\n",
            "GET / HTTP/1.1\r\nHost: 127.0.0.1:8123\r\n folded\r\n\r\n",
            "GET / HTTP/1.1\r\nHost: 127.0.0.1:8123\r\nX: a\u{0}b\r\n\r\n",
            "GET / HTTP/1.1\r\nHost: 127.0.0.1:8123\r\nContent-Length: -1\r\n\r\n",
            "GET / HTTP/1.1\r\nHost: 127.0.0.1:8123\r\nContent-Length: 1\r\nContent-Length: 1\r\n\r\nx",
            // No empty line yet: the head never ended.
            "GET / HTTP/1.1\r\nHost: 127.0.0.1:8123\r\n",
            // The body is shorter than its Content-Length.
            "POST /api HTTP/1.1\r\nHost: 127.0.0.1:8123\r\nContent-Length: 10\r\n\r\n{}",
        ] {
            assert_refused(&one(raw), "400 Bad Request");
        }
    }

    #[test]
    fn a_malformed_protocol_body_is_a_protocol_answer() {
        let r = one(&api("not json"));
        assert_eq!(r.status, "HTTP/1.1 200 OK");
        assert_eq!(
            r.text(),
            r#"{"id":null,"ok":false,"error":{"message":"Malformed request: not valid JSON.","line":null}}"#
        );
        // No Content-Length is an empty body, which is no JSON either.
        let r = one(&format!("POST /api HTTP/1.1\r\n{GOOD}\r\n"));
        assert!(r.text().contains("not valid JSON"));
    }

    #[test]
    fn transfer_encoding_is_not_implemented() {
        let raw =
            format!("POST /api HTTP/1.1\r\n{GOOD}Transfer-Encoding: chunked\r\n\r\n0\r\n\r\n");
        assert_refused(&one(&raw), "501 Not Implemented");
    }

    #[test]
    fn the_body_limit_at_and_past_its_bound() {
        let head = |n: usize| format!("POST /api HTTP/1.1\r\n{GOOD}Content-Length: {n}\r\n\r\n");
        assert_refused(&one(&head(MAX_BODY + 1)), "413 Content Too Large");
        assert_refused(&one(&head(9_000_000)), "413 Content Too Large");
        let huge =
            format!("POST /api HTTP/1.1\r\n{GOOD}Content-Length: 99999999999999999999999\r\n\r\n");
        assert_refused(&one(&huge), "413 Content Too Large");
        // At the bound the body is accepted: 8 MiB of spaces around `{}`
        // is one JSON object with no op.
        let mut raw = head(MAX_BODY).into_bytes();
        raw.extend(std::iter::repeat_n(b' ', MAX_BODY - 2));
        raw.extend_from_slice(b"{}");
        let r = parse(&handle(&raw, &mut fresh(), &cfg()));
        assert_eq!(r.status, "HTTP/1.1 200 OK");
        assert!(r.text().contains("no 'op' field"));
    }

    /// A GET whose head, blank line included, is exactly `n` bytes.
    fn head_of(n: usize) -> String {
        let fixed = "GET / HTTP/1.1\r\nHost: 127.0.0.1:8123\r\nX: \r\n\r\n".len();
        let filler = "a".repeat(n - fixed);
        format!("GET / HTTP/1.1\r\nHost: 127.0.0.1:8123\r\nX: {filler}\r\n\r\n")
    }

    #[test]
    fn the_head_limit_at_and_past_its_bound() {
        assert_eq!(head_of(MAX_HEAD).len(), MAX_HEAD);
        assert_eq!(one(&head_of(MAX_HEAD)).status, "HTTP/1.1 200 OK");
        assert_refused(
            &one(&head_of(MAX_HEAD + 1)),
            "431 Request Header Fields Too Large",
        );
        // A head with no end, past the cap, is too large rather than cut
        // short.
        let endless = format!("GET / HTTP/1.1\r\nX: {}", "a".repeat(MAX_HEAD));
        assert_refused(&one(&endless), "431 Request Header Fields Too Large");
    }

    #[test]
    fn same_secret_compares_whole_values() {
        assert!(same_secret(b"abc", b"abc"));
        assert!(!same_secret(b"abc", b"abd"));
        assert!(!same_secret(b"abc", b"ab"));
        assert!(!same_secret(b"", b"a"));
    }

    fn frame(input: &[u8], skip: bool) -> Option<Request> {
        read_request(&mut &input[..], skip).expect("in-memory reads cannot fail")
    }

    #[test]
    fn read_request_frames_by_content_length() {
        let two = format!("{}{}", api("{}"), get("/"));
        let mut input = two.as_bytes();
        let first = read_request(&mut input, false).unwrap().unwrap();
        assert_eq!(first.bytes, api("{}").as_bytes());
        assert!(first.head_complete);
        let second = read_request(&mut input, false).unwrap().unwrap();
        assert_eq!(second.bytes, get("/").as_bytes());
        assert_eq!(read_request(&mut input, false).unwrap(), None);
    }

    #[test]
    fn read_request_skips_empty_lines_only_when_asked() {
        let raw = format!("\n\r\n\n{}", get("/"));
        assert_eq!(
            frame(raw.as_bytes(), true).unwrap().bytes,
            get("/").as_bytes()
        );
        assert_eq!(frame(raw.as_bytes(), false).unwrap().bytes, b"\n");
        assert_eq!(frame(b"\n\n\r\n", true), None);
        assert_eq!(frame(b"", false), None);
    }

    #[test]
    fn read_request_stops_one_byte_past_the_head_cap() {
        let raw = format!("GET / HTTP/1.1\r\nX: {}\r\n\r\n", "a".repeat(3 * MAX_HEAD));
        let req = frame(raw.as_bytes(), false).unwrap();
        assert_eq!(req.bytes.len(), MAX_HEAD + 1);
        assert!(!req.head_complete);
        // A head at the cap exactly is read whole.
        let req = frame(head_of(MAX_HEAD).as_bytes(), false).unwrap();
        assert_eq!(req.bytes, head_of(MAX_HEAD).as_bytes());
        assert!(req.head_complete);
    }

    #[test]
    fn read_request_leaves_a_refused_body_unread() {
        let raw = format!("POST /api HTTP/1.1\r\n{GOOD}Content-Length: 9000000\r\n\r\nxyz");
        let mut input = raw.as_bytes();
        let req = read_request(&mut input, false).unwrap().unwrap();
        assert!(req.bytes.ends_with(b"\r\n\r\n"));
        assert_eq!(input, b"xyz");
    }

    /// Runs `input` through the stdio loop in one session.
    fn stdio(input: &[u8]) -> String {
        let mut out = Vec::new();
        serve_stdio(input, &mut out, &cfg()).expect("in-memory I/O cannot fail");
        String::from_utf8(out).expect("UTF-8 responses")
    }

    #[test]
    fn stdio_answers_each_request_and_separates_them_with_a_newline() {
        let a = api(r#"{"id":1,"op":"eval","code":"x = 1 + 2"}"#).replace("\r\n", "\n");
        let b = api(r#"{"id":2,"op":"eval","code":"disp(x * 2)"}"#).replace("\r\n", "\n");
        let out = stdio(format!("{a}\n{b}\n").as_bytes());
        let first = handle(
            api(r#"{"id":1,"op":"eval","code":"x = 1 + 2"}"#).as_bytes(),
            &mut fresh(),
            &cfg(),
        );
        assert!(out.starts_with(std::str::from_utf8(&first).unwrap()));
        assert_eq!(out.matches("HTTP/1.1 200 OK").count(), 2);
        assert!(out.ends_with("{\"id\":2,\"ok\":true,\"out\":\"     6\\n\"}\n"));
        assert!(out.contains("\\n\\n\"}\nHTTP/1.1 200 OK\r\n"));
    }

    #[test]
    fn stdio_reads_the_body_of_a_request_refused_after_framing() {
        // Two Host headers are refused, but the length is known, so the body
        // is read and the next request is framed where it starts.
        let twice = format!("{GOOD}Host: 127.0.0.1:8123\r\n");
        let next = api(r#"{"id":2,"op":"eval","code":"disp(7)"}"#);
        let out = stdio(format!("{}{next}", api_with(&twice)).as_bytes());
        assert!(out.starts_with("HTTP/1.1 400 Bad Request\r\n"), "{out}");
        assert_eq!(out.matches("HTTP/1.1 ").count(), 2, "{out}");
        assert!(
            out.ends_with("\r\n\r\n{\"id\":2,\"ok\":true,\"out\":\"     7\\n\"}\n"),
            "{out}"
        );
    }

    #[test]
    fn stdio_moves_on_after_an_oversized_head() {
        let long = format!(
            "GET / HTTP/1.1\nHost: 127.0.0.1:8123\nX: {}\nY: z\n\n",
            "a".repeat(2 * MAX_HEAD)
        );
        let out = stdio(format!("{long}{}", get("/nope")).as_bytes());
        let statuses: Vec<&str> = out.lines().filter(|l| l.starts_with("HTTP/1.1")).collect();
        assert_eq!(
            statuses,
            [
                "HTTP/1.1 431 Request Header Fields Too Large",
                "HTTP/1.1 404 Not Found"
            ]
        );
    }

    #[test]
    fn discard_head_stops_after_the_empty_line() {
        for (tail, rest, left) in [
            (&b"aaa"[..], &b"aa\r\nb: c\r\n\r\nNEXT"[..], &b"NEXT"[..]),
            (b"", b"\nNEXT", b"NEXT"),
            (b"\r", b"\nNEXT", b"NEXT"),
            (b"x", b"\n\nNEXT", b"NEXT"),
            (b"", b"no end", b""),
        ] {
            let mut input = rest;
            discard_head(&mut input, tail).unwrap();
            assert_eq!(input, left, "{:?}", String::from_utf8_lossy(rest));
        }
    }
}
