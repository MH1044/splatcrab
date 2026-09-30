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
//! sends one; `/api` also needs the session token in `X-SplatCrab-Token`,
//! since cycle U3 a `Sec-Fetch-Site` of `same-origin` when it has one, and
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

/// `Sec-Fetch-Site`, when present, is `same-origin` (cycle U3). A browser
/// adds it to every request a page makes, naming whether the page is this
/// server's own, so a request another site makes is refused even if it
/// somehow carried the token. A client that sends none, a golden case or a
/// script, is unaffected; two of them fail, as a repeated `Origin` does.
/// Values are compared in any case, trimmed already.
fn same_origin_fetch(head: &Head) -> bool {
    match head.single("Sec-Fetch-Site") {
        Ok(None) => true,
        Ok(Some(site)) => trim_ows(site).eq_ignore_ascii_case(b"same-origin"),
        Err(()) => false,
    }
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
        // After the token and before the content type; the static routes
        // are not checked, since a navigation to the page is not a
        // same-origin request.
        b"/api" if !same_origin_fetch(&head) => refuse(Status::Forbidden),
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

/// `splatcrab --http-stdio`: [`handle`] with no socket, over a fresh session
/// whose file root is fixed now, to the working directory, as `--ui` fixes
/// its own.
pub fn serve_stdio(input: impl BufRead, output: impl Write, cfg: &Config) -> io::Result<()> {
    let mut it = Interp::with_sinks(Box::new(io::sink()), Box::new(io::sink()));
    it.file_root = Some(crate::files::session_root());
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

    /// Every CSS named colour, CSS Color Module Level 4's list, in lower
    /// case. `transparent` and `currentColor` are keywords outside it, and
    /// allowed anywhere, as `inherit` is.
    const NAMED_COLOURS: [&str; 148] = [
        "aliceblue",
        "antiquewhite",
        "aqua",
        "aquamarine",
        "azure",
        "beige",
        "bisque",
        "black",
        "blanchedalmond",
        "blue",
        "blueviolet",
        "brown",
        "burlywood",
        "cadetblue",
        "chartreuse",
        "chocolate",
        "coral",
        "cornflowerblue",
        "cornsilk",
        "crimson",
        "cyan",
        "darkblue",
        "darkcyan",
        "darkgoldenrod",
        "darkgray",
        "darkgreen",
        "darkgrey",
        "darkkhaki",
        "darkmagenta",
        "darkolivegreen",
        "darkorange",
        "darkorchid",
        "darkred",
        "darksalmon",
        "darkseagreen",
        "darkslateblue",
        "darkslategray",
        "darkslategrey",
        "darkturquoise",
        "darkviolet",
        "deeppink",
        "deepskyblue",
        "dimgray",
        "dimgrey",
        "dodgerblue",
        "firebrick",
        "floralwhite",
        "forestgreen",
        "fuchsia",
        "gainsboro",
        "ghostwhite",
        "gold",
        "goldenrod",
        "gray",
        "green",
        "greenyellow",
        "grey",
        "honeydew",
        "hotpink",
        "indianred",
        "indigo",
        "ivory",
        "khaki",
        "lavender",
        "lavenderblush",
        "lawngreen",
        "lemonchiffon",
        "lightblue",
        "lightcoral",
        "lightcyan",
        "lightgoldenrodyellow",
        "lightgray",
        "lightgreen",
        "lightgrey",
        "lightpink",
        "lightsalmon",
        "lightseagreen",
        "lightskyblue",
        "lightslategray",
        "lightslategrey",
        "lightsteelblue",
        "lightyellow",
        "lime",
        "limegreen",
        "linen",
        "magenta",
        "maroon",
        "mediumaquamarine",
        "mediumblue",
        "mediumorchid",
        "mediumpurple",
        "mediumseagreen",
        "mediumslateblue",
        "mediumspringgreen",
        "mediumturquoise",
        "mediumvioletred",
        "midnightblue",
        "mintcream",
        "mistyrose",
        "moccasin",
        "navajowhite",
        "navy",
        "oldlace",
        "olive",
        "olivedrab",
        "orange",
        "orangered",
        "orchid",
        "palegoldenrod",
        "palegreen",
        "paleturquoise",
        "palevioletred",
        "papayawhip",
        "peachpuff",
        "peru",
        "pink",
        "plum",
        "powderblue",
        "purple",
        "rebeccapurple",
        "red",
        "rosybrown",
        "royalblue",
        "saddlebrown",
        "salmon",
        "sandybrown",
        "seagreen",
        "seashell",
        "sienna",
        "silver",
        "skyblue",
        "slateblue",
        "slategray",
        "slategrey",
        "snow",
        "springgreen",
        "steelblue",
        "tan",
        "teal",
        "thistle",
        "tomato",
        "turquoise",
        "violet",
        "wheat",
        "white",
        "whitesmoke",
        "yellow",
        "yellowgreen",
    ];

    /// The system colours, which name a colour just as a named colour
    /// does. All of them are checked in a stylesheet's values; the page and
    /// the script are checked for all but [`ORDINARY_WORDS`].
    const SYSTEM_COLOURS: [&str; 42] = [
        "accentcolor",
        "accentcolortext",
        "activetext",
        "buttonborder",
        "buttonface",
        "buttontext",
        "canvas",
        "canvastext",
        "field",
        "fieldtext",
        "graytext",
        "highlight",
        "highlighttext",
        "linktext",
        "mark",
        "marktext",
        "selecteditem",
        "selecteditemtext",
        "visitedtext",
        "activeborder",
        "activecaption",
        "appworkspace",
        "background",
        "buttonhighlight",
        "buttonshadow",
        "captiontext",
        "inactiveborder",
        "inactivecaption",
        "inactivecaptiontext",
        "infobackground",
        "infotext",
        "menu",
        "menutext",
        "scrollbar",
        "threeddarkshadow",
        "threedface",
        "threedhighlight",
        "threedlightshadow",
        "threedshadow",
        "window",
        "windowframe",
        "windowtext",
    ];

    /// The system colours that are also ordinary words of a script or a
    /// page (`window`, a `field`, a `mark`), which the page and script
    /// checks leave out; every other system colour they refuse.
    const ORDINARY_WORDS: [&str; 7] = [
        "background",
        "canvas",
        "field",
        "highlight",
        "mark",
        "menu",
        "window",
    ];

    /// The colour functions the spec names, and the two newer ones that
    /// make a colour as surely.
    const COLOUR_FUNCTIONS: [&str; 12] = [
        "rgb",
        "rgba",
        "hsl",
        "hsla",
        "hwb",
        "lab",
        "lch",
        "oklab",
        "oklch",
        "color",
        "color-mix",
        "light-dark",
    ];

    fn ident_byte(b: u8) -> bool {
        b.is_ascii_alphanumeric() || b == b'-' || b == b'_'
    }

    /// Every colour `text` names: a hexadecimal colour (`#` and 3, 4, 6 or
    /// 8 hex digits ending there), a colour function called, or a named
    /// colour or a system colour as a whole identifier, in any case. With
    /// `every_system` false, the system colours that are ordinary words
    /// ([`ORDINARY_WORDS`]) are not counted.
    fn colours_in(text: &str, every_system: bool) -> Vec<String> {
        let b = text.as_bytes();
        let mut found = Vec::new();
        let mut k = 0;
        while k < b.len() {
            if b[k] == b'#' {
                let digits = b[k + 1..]
                    .iter()
                    .take_while(|c| c.is_ascii_hexdigit())
                    .count();
                let end = k + 1 + digits;
                if [3, 4, 6, 8].contains(&digits) && !b.get(end).is_some_and(|&c| ident_byte(c)) {
                    found.push(text[k..end].to_string());
                }
                k = end.max(k + 1);
            } else if ident_byte(b[k]) {
                let start = k;
                while k < b.len() && ident_byte(b[k]) {
                    k += 1;
                }
                let word = text[start..k].to_ascii_lowercase();
                let called = b.get(k) == Some(&b'(');
                if (called && COLOUR_FUNCTIONS.contains(&word.as_str()))
                    || NAMED_COLOURS.contains(&word.as_str())
                    || (SYSTEM_COLOURS.contains(&word.as_str())
                        && (every_system || !ORDINARY_WORDS.contains(&word.as_str())))
                {
                    found.push(text[start..k].to_string());
                }
            } else {
                k += 1;
            }
        }
        found
    }

    /// `css` without its comments and with every string emptied, so
    /// neither can hide a brace or a semicolon from [`declarations`]. A
    /// string ends at its first quote that no backslash escapes, as CSS
    /// reads it, so `"a\""` cannot end early and swallow what follows it.
    fn strip_css(css: &str) -> String {
        let mut out = String::new();
        let mut chars = css.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '/' if chars.peek() == Some(&'*') => {
                    chars.next();
                    let mut last = ' ';
                    for d in chars.by_ref() {
                        if last == '*' && d == '/' {
                            break;
                        }
                        last = d;
                    }
                    out.push(' ');
                }
                '"' | '\'' => {
                    while let Some(d) = chars.next() {
                        if d == '\\' {
                            // The escaped character, a quote included, is
                            // part of the string.
                            chars.next();
                        } else if d == c || d == '\n' {
                            // CSS ends an unterminated string at the line
                            // end, so what follows is scanned, not hidden.
                            break;
                        }
                    }
                    out.push_str("\"\"");
                }
                c => out.push(c),
            }
        }
        out
    }

    /// A declaration with the preludes of the blocks around it.
    type Declaration = (Vec<String>, String);

    /// Every declaration of a stylesheet with the preludes of the blocks
    /// around it, outermost first, and every block's prelude path: what
    /// the palette rule is judged on.
    fn declarations(css: &str) -> (Vec<Declaration>, Vec<Vec<String>>) {
        let text = strip_css(css);
        let mut decls = Vec::new();
        let mut blocks = Vec::new();
        let mut stack: Vec<String> = Vec::new();
        let mut piece = String::new();
        for c in text.chars() {
            match c {
                '{' => {
                    stack.push(piece.split_whitespace().collect::<Vec<_>>().join(" "));
                    blocks.push(stack.clone());
                    piece.clear();
                }
                ';' | '}' => {
                    if !piece.trim().is_empty() {
                        decls.push((stack.clone(), piece.trim().to_string()));
                    }
                    piece.clear();
                    if c == '}' {
                        stack.pop();
                    }
                }
                c => piece.push(c),
            }
        }
        (decls, blocks)
    }

    const LIGHT: &str = ":root";
    const DARK: &str = "@media (prefers-color-scheme: dark)";

    /// What is wrong with a stylesheet's palette: every colour must be a
    /// custom property in the one `:root` rule (light) or the one `:root`
    /// rule of the one `prefers-color-scheme: dark` block, the two sets
    /// naming the same properties, and no other declaration may hold a
    /// colour. A backslash outside a string or a comment is refused
    /// outright: a CSS escape can spell a colour (`r\65 d` is `red`) that
    /// the scan would not see, and the page's own sheet has none. Empty
    /// when all of it holds.
    fn palette_problems(css: &str) -> Vec<String> {
        let (decls, blocks) = declarations(css);
        let mut problems = Vec::new();
        if strip_css(css).contains('\\') {
            problems.push("a backslash outside a string or a comment".to_string());
        }
        let count = |path: &[&str]| blocks.iter().filter(|b| *b == path).count();
        if count(&[LIGHT]) != 1 {
            problems.push(format!("{} top-level :root rules", count(&[LIGHT])));
        }
        if count(&[DARK]) != 1 || count(&[DARK, LIGHT]) != 1 {
            problems.push("not exactly one dark block with one :root rule".to_string());
        }
        let mut light = Vec::new();
        let mut dark = Vec::new();
        for (path, decl) in &decls {
            let (property, value) = decl.split_once(':').unwrap_or((decl, ""));
            let property = property.trim();
            let colours = colours_in(value, true);
            if colours.is_empty() {
                continue;
            }
            let palette = if path == &[LIGHT] {
                Some(&mut light)
            } else if path == &[DARK, LIGHT] {
                Some(&mut dark)
            } else {
                None
            };
            match palette {
                Some(set) if property.starts_with("--") => set.push(property.to_string()),
                _ => problems.push(format!("{path:?} {decl}: {colours:?}")),
            }
        }
        light.sort();
        dark.sort();
        if light != dark {
            problems.push(format!("light {light:?} and dark {dark:?} differ"));
        }
        problems
    }

    /// Cycle U2: the palette is defined exactly once. Every colour of
    /// `app.css` is a custom property of the light or the dark `:root`
    /// rule, and the page and its script name no colour at all.
    #[test]
    fn the_palette_is_defined_once() {
        assert_eq!(palette_problems(STYLE), Vec::<String>::new());
        for (name, text) in [("index.html", PAGE), ("app.js", SCRIPT)] {
            assert_eq!(colours_in(text, false), Vec::<String>::new(), "{name}");
        }
        // The page's sizes come from script through custom properties, never
        // a style attribute.
        assert!(SCRIPT.contains("style.setProperty('--left'"));
        assert!(!SCRIPT.contains("innerHTML") && !SCRIPT.contains("outerHTML"));
        assert!(!SCRIPT.contains("setAttribute('style'"));
    }

    /// The rule's teeth: each of these sheets breaks it one way, and each
    /// is refused; the keywords that name no colour of their own pass.
    #[test]
    fn a_stylesheet_that_breaks_the_palette_is_refused() {
        const GOOD: &str = ":root { --a: #fff; --b: rgb(0 0 0); --font: \"Menlo\"; }\n\
             @media (prefers-color-scheme: dark) { :root { --a: #111; --b: black; } }\n\
             a { color: var(--a); border: 1px solid transparent; fill: currentColor; \
             background: inherit; white-space: pre-wrap; }\n\
             #bad2 { content: \"red\"; quotes: \"\\\"\" 'it\\'s'; } /* a comment may say red */";
        assert_eq!(palette_problems(GOOD), Vec::<String>::new());
        assert_eq!(NAMED_COLOURS.len(), 148);
        let dark = "@media (prefers-color-scheme: dark) { :root { --a: #111; } }";
        for bad in [
            format!(":root {{ --a: #fff; }} {dark} a {{ color: red; }}"),
            format!(":root {{ --a: #fff; }} {dark} a {{ color: #abc; }}"),
            format!(":root {{ --a: #fff; }} {dark} a {{ color: #AABBCCDD; }}"),
            format!(":root {{ --a: #fff; }} {dark} a {{ background: rgb(0 0 0); }}"),
            format!(":root {{ --a: #fff; }} {dark} a {{ color: HSL(0 0% 0%); }}"),
            format!(":root {{ --a: #fff; }} {dark} a {{ border: 1px solid ForestGreen; }}"),
            format!(":root {{ --a: #fff; }} {dark} a {{ color: Canvas; }}"),
            format!(
                ":root {{ --a: #fff; }} {dark} a {{ color: color-mix(in srgb, var(--a), var(--a)); }}"
            ),
            // A colour in the light rule that is not a custom property.
            format!(":root {{ --a: #fff; color: #000; }} {dark}"),
            // A colour inside another media block.
            format!(
                ":root {{ --a: #fff; }} {dark} @media (max-width: 700px) {{ a {{ color: #123; }} }}"
            ),
            // A colour in a second :root rule.
            format!(":root {{ --a: #fff; }} {dark} :root {{ --b: #000; }}"),
            // Light and dark name different properties.
            format!(":root {{ --a: #fff; --b: #000; }} {dark}"),
            // No dark block, or two.
            ":root { --a: #fff; }".to_string(),
            format!(":root {{ --a: #fff; }} {dark} {dark}"),
            // An escaped quote does not end a string early, so the string
            // cannot swallow the colour after it (cycle U2's review).
            format!(r#":root {{ --a: #fff; }} {dark} a {{ content: "a\""; color: red; }}"#),
            format!(r":root {{ --a: #fff; }} {dark} a {{ content: 'it\'s'; color: red; }}"),
            // A string left open ends at its line end, as CSS ends it, so
            // it cannot hide the colour on the next line.
            format!(":root {{ --a: #fff; }} {dark} a {{ content: \"x\n; color: red; }}"),
            // A CSS escape spelling a colour, `r\65 d` for `red`, and any
            // other backslash outside a string.
            format!(r":root {{ --a: #fff; }} {dark} a {{ color: r\65 d; }}"),
            format!(r":root {{ --a: #fff; }} {dark} a\{{ }} b {{ color: var(--a); }}"),
        ] {
            assert!(!palette_problems(&bad).is_empty(), "accepted: {bad}");
        }
        // And the page and script checks see a colour wherever one is,
        // system colours included where they are not ordinary words.
        for bad in [
            "x.style.color = 'red';",
            "// #fff",
            "rgba(0,0,0,0)",
            "<b class=\"navy\">",
            "x.style.color = 'ButtonText';",
            "GrayText",
            "canvastext",
            "<i class=\"AccentColor\">",
        ] {
            assert!(!colours_in(bad, false).is_empty(), "{bad}");
        }
        for fine in [
            "#desktop",
            "#input",
            "white-space",
            "--left",
            "window.color_scheme",
            "Field",
            "mark",
            "Menu",
            "background",
            "highlight",
            "canvas",
        ] {
            assert_eq!(colours_in(fine, false), Vec::<String>::new(), "{fine}");
        }
        // Every system colour but the seven ordinary words is refused in
        // the page and the script, and all of them in a stylesheet.
        for word in SYSTEM_COLOURS {
            assert_eq!(
                colours_in(word, false).is_empty(),
                ORDINARY_WORDS.contains(&word),
                "{word}"
            );
            assert_eq!(colours_in(word, true), [word], "{word}");
        }
    }

    /// Where `app.js`'s function `name` is, from its `function` keyword to
    /// its closing brace, found by counting braces from the first one after
    /// the signature: the functions it is asked for hold no brace in a
    /// string or a comment. The script must define the name exactly once.
    fn function_span(script: &str, name: &str) -> std::ops::Range<usize> {
        let signature = format!("function {name}(");
        assert_eq!(script.matches(&signature).count(), 1, "one {signature}");
        let start = script.find(&signature).unwrap_or_default();
        let open = start + script[start..].find('{').expect("a body");
        let mut depth = 0;
        for (k, c) in script[open..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return start..open + k + 1;
                    }
                }
                _ => {}
            }
        }
        panic!("the body of {name} is never closed");
    }

    /// Every byte offset where `word` stands in `text` as a whole
    /// JavaScript identifier.
    fn word_offsets(text: &str, word: &str) -> Vec<usize> {
        let ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'$';
        text.match_indices(word)
            .map(|(k, _)| k)
            .filter(|&k| {
                let before = text.as_bytes()[..k].last().copied();
                let after = text.as_bytes().get(k + word.len()).copied();
                !before.is_some_and(ident) && !after.is_some_and(ident)
            })
            .collect()
    }

    /// Cycle U2: every request goes through the one queue. `app.js` names
    /// `fetch` exactly once, called inside the queue's `send`, and names
    /// `send` only where it is defined and inside `call`, which chains each
    /// request on the one before; no other way to reach the network
    /// appears in the script at all.
    #[test]
    fn every_request_goes_through_the_one_queue() {
        let send = function_span(SCRIPT, "send");
        let call = function_span(SCRIPT, "call");
        let fetches = word_offsets(SCRIPT, "fetch");
        assert_eq!(fetches.len(), 1, "fetch is named once");
        assert_eq!(SCRIPT.matches("fetch(").count(), 1);
        assert!(SCRIPT[fetches[0]..].starts_with("fetch("));
        assert!(send.contains(&fetches[0]), "fetch is called in send");
        let defined = send.start + "function ".len();
        let sends = word_offsets(SCRIPT, "send");
        assert!(sends.contains(&defined));
        for k in &sends {
            assert!(
                *k == defined || call.contains(k),
                "send named outside call at byte {k}"
            );
        }
        assert!(sends.len() >= 2, "call calls send");
        assert!(SCRIPT[call.clone()].contains("queue.then("));
        assert!(SCRIPT[call].contains("return send(request);"));
        for other in [
            "XMLHttpRequest",
            "sendBeacon",
            "WebSocket",
            "EventSource",
            "import(",
        ] {
            assert!(!SCRIPT.contains(other), "{other}");
        }
    }

    /// Cycle U3: the editor asks its questions in the page, never in a
    /// dialog of the browser's; the markup has no inline handler; every
    /// `eval` the page makes asks for the stack, and Run is `run_file`;
    /// leaving with unsaved tabs is the browser's own warning; and user text
    /// reaches the page as text.
    #[test]
    fn the_editor_keeps_to_the_page_and_its_text() {
        for dialog in ["confirm(", "prompt(", "alert(", "showModalDialog"] {
            assert!(!SCRIPT.contains(dialog), "{dialog}");
        }
        for text in [
            "innerHTML",
            "outerHTML",
            "insertAdjacentHTML",
            "document.write",
            "eval(",
            "new Function",
        ] {
            assert!(!SCRIPT.contains(text), "{text}");
        }
        // No inline handler: no attribute `on<letters>=` in the markup.
        let page = PAGE.as_bytes();
        for (k, _) in PAGE.match_indices(" on") {
            let rest = &page[k + 3..];
            let letters = rest.iter().take_while(|b| b.is_ascii_alphabetic()).count();
            assert!(
                letters == 0 || rest.get(letters) != Some(&b'='),
                "an inline handler at byte {k}"
            );
        }
        // Every eval asks for its stack.
        let evals: Vec<usize> = SCRIPT.match_indices("op: 'eval'").map(|(k, _)| k).collect();
        assert!(!evals.is_empty());
        for k in evals {
            let object = &SCRIPT[k..k + SCRIPT[k..].find('}').expect("the request's end")];
            assert!(object.contains("stack: true"), "{object}");
        }
        for op in ["op: 'read_file'", "op: 'write_file'", "op: 'run_file'"] {
            assert!(SCRIPT.contains(op), "{op}");
        }
        assert!(SCRIPT.contains("'beforeunload'"));
        assert!(SCRIPT.contains("style.setProperty('--editor'"));
        assert!(SCRIPT.contains("style.setProperty('--mark-row'"));
        // The fourth splitter, as U2's three.
        assert!(PAGE.contains(
            r#"id="split-mid" class="splitter" role="separator" aria-orientation="horizontal""#
        ));
        assert!(PAGE.contains(r#"<textarea id="code" wrap="off""#));
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
            assert!(it.vars().is_empty(), "{headers}");
        }
        assert_eq!(send(&mut it, &api_with(GOOD)).status, "HTTP/1.1 200 OK");
        assert!(it.vars().contains_key("x"));
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

    /// Cycle U3: a `Sec-Fetch-Site` other than `same-origin` is `403` and
    /// never reaches the interpreter, judged after the token and before the
    /// content type; none at all is fine, and the static routes are not
    /// checked.
    #[test]
    fn sec_fetch_site_when_present_must_be_same_origin() {
        let mut it = fresh();
        for site in [
            "cross-site",
            "same-site",
            "none",
            "Cross-Site",
            "same-origin-x",
            "",
        ] {
            let headers = format!("{GOOD}Sec-Fetch-Site: {site}\r\n");
            assert_refused(&send(&mut it, &api_with(&headers)), "403 Forbidden");
            assert!(it.vars().is_empty(), "{site}");
        }
        let twice = format!("{GOOD}Sec-Fetch-Site: same-origin\r\nSec-Fetch-Site: same-origin\r\n");
        assert_refused(&send(&mut it, &api_with(&twice)), "403 Forbidden");
        assert!(it.vars().is_empty());
        // Refused before the content type is looked at, and after the
        // token, whose refusal is the same status.
        let plain = GOOD.replace("application/json", "text/plain");
        let headers = format!("{plain}Sec-Fetch-Site: cross-site\r\n");
        assert_refused(&one(&api_with(&headers)), "403 Forbidden");
        let headers = format!("{plain}Sec-Fetch-Site: same-origin\r\n");
        assert_refused(&one(&api_with(&headers)), "415 Unsupported Media Type");
        for site in [
            "same-origin",
            "Same-Origin",
            "SAME-ORIGIN",
            " same-origin\t",
        ] {
            let headers = format!("{GOOD}sec-fetch-site:{site}\r\n");
            assert_eq!(
                one(&api_with(&headers)).status,
                "HTTP/1.1 200 OK",
                "{site:?}"
            );
        }
        assert_eq!(send(&mut it, &api_with(GOOD)).status, "HTTP/1.1 200 OK");
        assert!(it.vars().contains_key("x"));
        // A navigation to the page is not same-origin, and is served.
        for path in ["/", "/app.js", "/app.css"] {
            let raw = format!(
                "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:8123\r\nSec-Fetch-Site: none\r\n\r\n"
            );
            assert_eq!(one(&raw).status, "HTTP/1.1 200 OK", "{path}");
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
