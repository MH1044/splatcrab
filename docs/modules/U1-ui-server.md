# U1 — UI server

## Goal

`splatcrab --ui` serves a page on the loopback interface that runs what the
user types and shows its exact output, speaking cycle U0's protocol over HTTP.
The HTTP handling is a pure function from request bytes to response bytes,
so the golden harness pins it through a stdio mode exactly as it pins U0, and
the socket around it is a thin loop that one integration test exercises for
real. The standard library only: the server, the HTTP parsing and the page
are all written here.

After this module a user can do things that were not possible before; every
bullet in Scope must be demonstrable by at least one acceptance test below.

## Scope

- **`splatcrab --ui`** binds `127.0.0.1` only, never a wildcard address, on
  the port `--port N` names or, by default, one the operating system picks.
  It prints one line to stdout, `SplatCrab UI: http://127.0.0.1:<port>/#<token>`,
  flushes it, and serves until killed. It tries to open that URL in the
  default browser (`cmd /c start`, `open`, `xdg-open`, by platform), ignores
  any failure to do so, and skips the attempt with `--no-browser`
- **The session token.** Each run makes a fresh 128-bit token, printed as 32
  lower-case hex digits, from the standard library's randomly keyed hasher
  mixed with the time and the process id; `--token T` sets it instead, for
  tests. It travels in the URL's fragment, which a browser never sends to a
  server or a `Referer`, and the page sends it back on every API call in an
  `X-SplatCrab-Token` header
- **Why the token, and the other checks.** Any web page the user visits can
  send requests to a loopback port. A custom header makes a cross-origin
  request need a CORS preflight, which this server never approves, and the
  token makes a guess useless. Beyond that, every request's `Host` must be
  `127.0.0.1:<port>` or `localhost:<port>`, which defeats DNS rebinding, and
  an `Origin`, when present, must be `http://127.0.0.1:<port>` or
  `http://localhost:<port>`. A request failing any check is `403 Forbidden`
  and never reaches the interpreter
- **Routes.** `GET /` is the page; `GET /app.js` and `GET /app.css` are its
  script and stylesheet, kept out of the page so the page's
  `Content-Security-Policy` can forbid inline script. `POST /api` takes one
  U0 request as its body, `Content-Type: application/json`, and answers with
  exactly the JSON line `protocol::respond` produces, less its newline. An
  unknown path is `404 Not Found`; a known path with the wrong method is
  `405 Method Not Allowed` with an `Allow` header; `/api` with another
  content type is `415 Unsupported Media Type`. Static routes need no token:
  they hold nothing secret
- **HTTP/1.1, the part the page uses.** A request line, headers up to a blank
  line, and a body of exactly `Content-Length` bytes. Header names are
  case-insensitive. Both CRLF and a bare LF end a line, since a stdio case is
  easier to write with LF. Chunked bodies, `Expect: 100-continue` and
  keep-alive are not supported: every response carries `Connection: close`
  and the server closes the connection after writing it. A malformed request
  is `400 Bad Request`; a `Transfer-Encoding` it does not support is
  `501 Not Implemented`
- **Limits.** The request line and headers together are capped at 16 KiB
  (`431 Request Header Fields Too Large`), and a body at 8 MiB (`413 Content
  Too Large`), both judged before the bytes are buffered, so no request can
  make the server allocate without bound. A socket read that stalls for 10
  seconds closes the connection. This is the line-length cap U0's Design
  notes asked for before a socket went in front of `serve`
- **Responses are deterministic.** The status line, then these headers in
  this order: `Content-Type`, `Content-Length`, `Cache-Control: no-store`,
  `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`, the
  page's `Content-Security-Policy: default-src 'self'; frame-ancestors
  'none'` on `GET /` only, `Allow` on a 405 only, then `Connection: close`
  and a blank line. No `Date`, no `Server`. An error status has a short
  `text/plain; charset=utf-8` body naming it, for example `403 Forbidden`,
  with every text in `src/error.rs`
- **`src/http.rs`**, the pure layer: `handle(request_bytes, &mut Interp,
  &Config) -> Vec<u8>`, where `Config` holds the port and the token. Unit
  tests drive it with byte strings
- **`src/server.rs`**, the socket loop: accept, read one request under the
  limits and the timeout, call `http::handle`, write, close. One connection
  at a time, on the interpreter thread, since the session is one `Interp`
- **`splatcrab --http-stdio --port N --token T`** reads HTTP requests one
  after another from stdin, each ending where its `Content-Length` says, and
  writes each response to stdout: `http::handle` with no socket. It exits 0
  at end of input. It is what the golden cases drive
- **An `.http` golden case kind**, like `.proto`: the harness spawns the
  binary with `--http-stdio --port 8123 --token test-token` and pipes the file,
  less its `% covers:` line, to stdin. Documented in `docs/TESTING.md` and at
  the top of `tests/golden.rs`
- **`tests/ui_server.rs`**, an integration test of the real socket: it spawns
  `--ui --port 0 --no-browser --token itest`, reads the printed URL, and over
  `std::net::TcpStream` runs an `eval` through `/api`, is refused without the
  token and with a foreign `Host`, and fetches `/`. It kills the child at the
  end, including when an assertion fails
- **The page**, embedded with `include_str!` from `src/ui/`: a command window.
  An input at the bottom and a transcript above it, in which each entry shows
  what was typed and then its output exactly, whitespace included, in a
  monospaced block, with an error set apart. Enter runs the entry when the
  server's `complete` says it is finished and inserts a newline otherwise.
  Up and Down walk this page's history. Light and dark follow the system. No
  external resources, no framework
- The REPL, script mode and `--protocol` behave exactly as before

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.
- Panes, the workspace view, the file browser and the palette defined once:
  cycle U2. The editor: U3. Figures and a shared working directory: U4.
- HTTPS, other hosts, and any address but loopback.
- Interrupting a running evaluation and streaming its output, as in U0.
- More than one browser tab sharing one session safely. Two tabs talk to the
  one interpreter, one request at a time; nothing stops that, and nothing
  more is promised.
- An automated browser test. The page's behaviour is checked by hand at the
  close of the cycle, and the commit says what was checked.

## Design notes

Filled in as decisions are made. Start from the
"Key designs to preserve" section of docs/ARCHITECTURE.md and record here:
which files and types changed, which invariants were preserved (column-major
storage, 1-based to 0-based conversion at the index boundary, the `end` stack,
name resolution order, the output sink), and any deviation from this spec
that was accepted deliberately.

Settled at planning:

- HTTP handling is a function of bytes, so every status and header is pinned
  by golden cases without a socket; the socket loop is small enough for one
  integration test to cover.
- The token rides in the URL fragment rather than the query string, so it
  never appears in a request line, a server log or a `Referer`.
- A custom header rather than a cookie, so that no browser sends the token on
  its own to a request some other page started.
- `Connection: close` on everything keeps the parser free of keep-alive and
  pipelining, at the cost of one connection per request, which a single
  person typing at a prompt will not notice.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/U1-ui-server/`, as an `.http` case unless it says otherwise.
Every case runs with port 8123 and token `test-token`. Expected output
follows from this spec's header order and the U0 protocol's recorded bytes;
a response's `Content-Length` is the byte count of the body the spec fixes.

1. `POST /api` with `Host: 127.0.0.1:8123`, `X-SplatCrab-Token: test-token`, `Content-Type: application/json` and the body `{"id":1,"op":"eval","code":"x = 1 + 2"}` → `HTTP/1.1 200 OK`, the headers in the spec's order with `Content-Type: application/json` and `Content-Length: 44` (counted from the binary's U0 response), and the body `{"id":1,"ok":true,"out":"x =\n\n     3\n\n"}`
2. Two requests in one case, the second `{"id":2,"op":"eval","code":"disp(x * 2)"}`, → the second body is `{"id":2,"ok":true,"out":"     6\n"}`: the session persists across requests
3. The same request with no token, then with `X-SplatCrab-Token: wrong`, → `403 Forbidden` both times, and a following valid `disp(x)` request shows `x` was never assigned: a refused request never reaches the interpreter
4. `Host: evil.example:8123`, and separately `Host: 127.0.0.1:9999`, with the right token → `403 Forbidden`; `Host: localhost:8123` → `200 OK`
5. `Origin: http://evil.example` with the right token and host → `403 Forbidden`; `Origin: http://127.0.0.1:8123` → `200 OK`
6. `GET /nope` → `404 Not Found`; `GET /api` → `405 Method Not Allowed` with `Allow: POST`; `POST /` → `405` with `Allow: GET`
7. `POST /api` with `Content-Type: text/plain` → `415 Unsupported Media Type`, even with the right token
8. A request line that is not HTTP, `hello`, → `400 Bad Request`; `Transfer-Encoding: chunked` → `501 Not Implemented`
9. A `Content-Length` of `9000000` → `413 Content Too Large`, answered without reading a body; headers past 16 KiB → `431 Request Header Fields Too Large`. Generate the long case with a script and the Write tool, as U0's deep-nesting case was
10. A malformed protocol body in a well-formed request, `not json`, → `200 OK` with the U0 malformed-request answer as its body: HTTP succeeded, and the protocol answered
11. Header names in any case: `host:`, `x-splatcrab-token:` and `CONTENT-TYPE:` → `200 OK`
12. `tests/ui_server.rs` (an integration test, not a golden case): over a real socket, an eval round trip, a 403 without the token, a 403 for a foreign `Host`, and `GET /` returning `200` with the page's `Content-Security-Policy`
13. Unit tests in `src/http.rs`: every status above, header order, `Content-Length` equal to the body's byte count on every response, the static routes' types and bodies equal to the embedded files, and the size limits at and past their bounds
14. The REPL, script and `.proto` cases pass unchanged

## Status

Planned
