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
  at end of input. It is what the golden cases drive. Two stdio-only rules,
  settled at planning so a case can be written by hand: empty lines before
  a request line are skipped (RFC 9112 asks a server to ignore at least one,
  and the LF that ends a hand-written body is one), and each response is
  followed by one `\n` that is a separator in the stdio stream and not part
  of the response, so a JSON body ends its line instead of running into the
  next status line. The socket writes the response alone
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

Recorded during implementation.

**Files and types.** New: `src/http.rs` (`handle`, `Config { port, token }`,
`Status`, `read_request` and `Request { bytes, head_complete }`,
`serve_stdio`/`serve_stdio_with`, the constants `MAX_HEAD`, `MAX_BODY`,
`CSP`, `TOKEN_HEADER`, and `PAGE`, `SCRIPT`, `STYLE` embedded from
`src/ui/`), `src/server.rs` (`bind`, `url`, `new_token`, `open_browser`,
`serve`, `TIMEOUT`), `src/ui/index.html`, `src/ui/app.js`, `src/ui/app.css`
and `tests/ui_server.rs`. Changed: `src/main.rs` dispatches `--ui` and
`--http-stdio` and parses their options; `src/lib.rs` exposes `http` and
`server`; `src/error.rs` gains the `HTTP_STATUS` table, `http_status`, the
option and listening messages, and scans the two new files for inline
message text; `tests/golden.rs` gains the `.http` kind and reads a case's
stdin as bytes; `.gitattributes` forces LF for `.http`.

**Invariants.** Nothing in the evaluator changed, so column-major storage,
the one-based conversion in `eval_index_args`, the `end` stack and the name
resolution order are untouched. The output sink is untouched too: both
`--ui` and `--http-stdio` build their `Interp` over `io::sink()`, and output
reaches a client only inside a `protocol::respond` answer, exactly as in U0.
`print!` stays in `src/main.rs`, which prints the URL line. Invariant 6: the
head parser does not recurse, `read_request` reads the head a line at a time
under a budget of `MAX_HEAD + 1` bytes and reads a body only after its
`Content-Length` has passed `MAX_BODY`, and `discard_head` drops the rest of
an oversized head through a fixed buffer without keeping it. The largest
request ever held is 16 KiB of head plus 8 MiB of body.

**Choices where the spec was silent.**

- The order of the checks, so that each status is unambiguous: framing
  (`431` for a head past the cap, `400` for one that never ends within it),
  the request line (`400`), header syntax (`400`), `Transfer-Encoding`
  (`501`), a bad or repeated `Content-Length` (`400`), a `Content-Length`
  past the cap (`413`); then a second `Host` (`400`) and a body shorter than
  its length (`400`); then `Host` and `Origin` (`403`) for every route; then
  the route (`404`, `405`); and for `POST /api` the token (`403`) and the content type
  (`415`). So a malformed request is `400` whoever sends it, a foreign `Host`
  is `403` even on a static route or an unknown path, and `GET /api` is `405`
  with or without a token.
- The head limit counts the request line and headers with their line ends
  and the empty line that ends them: a head of exactly 16,384 bytes is
  accepted and 16,385 is `431`. A body of exactly 8,388,608 bytes is
  accepted and a `Content-Length` of one more is `413`. A `Content-Length`
  too long for any integer is `413`, not `400`: it is a well-formed number,
  and too large.
- A missing `Host` fails the `Host` check (`403`); two `Host` headers are
  `400`, as RFC 9112 asks. A repeated `Origin`, `X-SplatCrab-Token` or
  `Content-Type` fails its check. `Host` and `Origin` compare
  case-insensitively, since host names do; the token compares exactly.
- The token comparison is constant-time for a given length (an OR of XORs
  over every byte), so a refusal's timing says nothing about how much of a
  guess was right; only the length can leak, and the token's length is not
  secret.
- The content type is `application/json` in any case, with or without
  parameters such as `; charset=utf-8`; none at all is `415`.
- A `POST /api` without `Content-Length` has an empty body, as RFC 9112
  says, which the protocol answers as not valid JSON. `411 Length Required`
  is not among the statuses the spec lists.
- Only the origin form of a target is accepted: one that does not start
  with `/` is `400`. A query string is ignored in routing, so `/?x=1` is the
  page. `HTTP/1.0` and `HTTP/1.1` are the versions accepted; anything else
  in the version field is `400`.
- A header line that starts with whitespace (an obsolete folding), has no
  colon, has whitespace or any other non-token byte in its name, or has a
  control character other than tab in its value is `400`. Values are trimmed
  of spaces and tabs.
- `Expect: 100-continue` is ignored, not refused: the body is read as sent,
  which is what a client that waits for `100` and then sends anyway expects.
- The static routes' types are `text/html; charset=utf-8`, `text/javascript;
  charset=utf-8` and `text/css; charset=utf-8`. A method other than `GET` on
  them, `HEAD` and `OPTIONS` included, is `405` with `Allow: GET`; any method
  but `POST` on `/api` is `405` with `Allow: POST`, which is how a CORS
  preflight is never approved.
- An error body is the status text alone, `403 Forbidden`, with no newline,
  the same string as the status line's after `HTTP/1.1 `. Both come from
  `error::HTTP_STATUS`.
- Framing depends only on what comes before the second `Host` check: the
  request line, header syntax, `Transfer-Encoding` and `Content-Length`.
  Whenever those pass, the body is read, whatever is refused after it, so a
  stream of requests stays in step. (Settled in testing: two `Host` headers
  had been refused before the body was read, which in `--http-stdio` left
  the body to be read as the next request line.)
- In `--http-stdio`, a head cut off by the cap is answered `431` and the rest
  of it is then read and dropped through its empty line, so the next request
  in the stream is read normally; its body, if it has one, is not read,
  since its length is in a header nobody parsed. A `413` leaves its body
  unread in the same way. The socket needs neither rule: it closes after
  every response.
- The 10 seconds are a deadline on the whole request, from `accept` to its
  last byte, not a timeout on each read: each read waits only for what is
  left of it. A read that stalls 10 seconds is caught, as Scope says, and so
  is a client that sends a byte every second, which a per-read timeout never
  catches and which would hold the one connection the server serves for as
  long as it liked. (Settled in testing, where a probe found exactly that.)
  Writing the response gets a deadline of its own, 10 seconds from its
  start, since the evaluation's time is not the client's, so a client that
  stops reading cannot hold the server either. After writing, it half-closes
  and drains what the client may still be sending, under one deadline of a
  second and at most 1 MiB, before closing, so the client reads the answer
  rather than a reset (closing with unread input makes the operating system
  send RST).
- A client that connects and closes without sending a byte gets no
  response. Any failure on one connection, a timeout included, drops that
  connection and the loop accepts the next; an `accept` that fails pauses
  50 ms rather than spinning.
- The token: two SipHash values from two `RandomState`s, whose keys come
  from the operating system's random source, each over the time in
  nanoseconds, the process id and the half's index, written as 16 hex digits
  each. `--token T` may hold letters, digits and `.`, `_`, `~`, `-` only: it
  travels in a URL fragment and a header unchanged, and on Windows through
  `cmd /C start`, which reads `&`, `|`, `^`, `<`, `>` and `%` as its own
  syntax, so `--token a&calc` would have started a program (narrowed at
  review from "printable ASCII").
- Options: `--port`, `--token` and `--no-browser` follow `--ui` in any order;
  `--http-stdio` takes `--port` and `--token`, both required, and refuses
  `--no-browser`. A bad port, a missing value, an unknown option and a port
  that cannot be bound are `Error: <msg>` on stderr and exit 1, with the
  texts in `error.rs`. No golden case kind can pass other flags, so
  `tests/ui_server.rs` pins these instead.
- The browser launcher runs with its standard streams null and is reaped
  on a thread of its own.
- The page: Enter with Shift always inserts a newline; Enter on an entry of
  nothing but whitespace does nothing; a run entry is added to the history
  unless it repeats the last one; Up recalls only from the input's first
  line and Down only from its last, so the arrows still move the cursor
  inside a multi-line entry; the input is read-only while a request is in
  flight, so one request at a time leaves the page. A transport refusal
  (a `403`, say) is shown as the server's status text in the error style,
  and a page opened without a token says so.

Settled in testing, for the golden cases, against the rules above:

- Error bodies are the status text with no newline, so their lengths are:
  `400 Bad Request` 15, `403 Forbidden` 13, `404 Not Found` 13,
  `405 Method Not Allowed` 22, `413 Content Too Large` 21,
  `415 Unsupported Media Type` 26, `431 Request Header Fields Too Large` 35,
  `501 Not Implemented` 19. In `--http-stdio` each is followed by the one
  `\n` separator, so the next status line starts a line of its own.
- After a `431` in `--http-stdio` the rest of the oversized head is dropped
  through its empty line and reading goes on; a case that ends there
  answers the `431` alone and exits 0. After a `413` or a `501` the body is
  not read, so a case puts such a request last.
- `GET /api` is `405` before any token or content-type check, and a
  `Transfer-Encoding` is `501` whether or not a `Content-Length` came with
  it, as the order above says.
- A missing `Host` is `403`, and a `Host` or `Origin` compares
  case-insensitively (`LOCALHOST:8123` is this server); a content type of
  `Application/JSON; charset=utf-8` is JSON; spaces and tabs around a header
  value are not part of it.

**Deviations.** None from Scope. One gap to note for the hand check at the
close of the cycle: one connection at a time means a connection a browser
opens and then leaves idle holds the server until it is used or the
10-second timeout closes it. Browsers reuse such a connection for their next
request, and every response closes its own, so this is not expected to show;
if it does, the fix is a reader thread per connection feeding the
interpreter thread, which is a change to Scope.

**Known limits, accepted at review.** None lets a web page run code, read
output or learn the token; each is outside the threat model above, which is
web pages, and each is recorded so that a later cycle can close it on
purpose:

- On Linux and macOS the URL, token included, is an argument of the
  `xdg-open` or `open` that starts the browser, and often of the browser
  itself, so another local user can read it from the process table. Windows
  keeps other users from reading it. Jupyter's answer is a redirect file only
  the user can read; `--no-browser` avoids it today.
- One connection at a time means a page that finds the port can hold the
  server with idle connections, ten seconds each, again and again: it can
  deny service, not use it. A reader thread per connection would close it,
  which is a Scope change.
- A panic inside an evaluation ends the server with exit 101, as it ends
  `--protocol`. Invariant 6 is what prevents it, not the server.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/U1-ui-server/`, as an `.http` case unless it says otherwise.
Every case runs with port 8123 and token `test-token`. Expected output
follows from this spec's header order and the U0 protocol's recorded bytes;
a response's `Content-Length` is the byte count of the body the spec fixes.

1. `POST /api` with `Host: 127.0.0.1:8123`, `X-SplatCrab-Token: test-token`, `Content-Type: application/json` and the body `{"id":1,"op":"eval","code":"x = 1 + 2"}` → `HTTP/1.1 200 OK`, the headers in the spec's order with `Content-Type: application/json` and `Content-Length: 44` (counted from the binary's U0 response), and the body `{"id":1,"ok":true,"out":"x =\n\n     3\n\n"}`. Cases: eval_round_trip.
2. Two requests in one case, the second `{"id":2,"op":"eval","code":"disp(x * 2)"}`, → the second body is `{"id":2,"ok":true,"out":"     6\n"}`: the session persists across requests. Cases: session_persists_across_requests.
3. The same request with no token, then with `X-SplatCrab-Token: wrong`, → `403 Forbidden` both times, and a following valid `disp(x)` request shows `x` was never assigned: a refused request never reaches the interpreter. Cases: err_token_missing_never_evaluated, err_token_wrong_never_evaluated.
4. `Host: evil.example:8123`, and separately `Host: 127.0.0.1:9999`, with the right token → `403 Forbidden`; `Host: localhost:8123` → `200 OK`. Cases: err_host_foreign_never_evaluated, err_host_wrong_port_never_evaluated, host_localhost_accepted, err_host_foreign_static_route.
5. `Origin: http://evil.example` with the right token and host → `403 Forbidden`; `Origin: http://127.0.0.1:8123` → `200 OK`. Cases: err_origin_foreign_never_evaluated, err_origin_wrong_port_never_evaluated, err_origin_null_never_evaluated, origin_loopback_accepted.
6. `GET /nope` → `404 Not Found`; `GET /api` → `405 Method Not Allowed` with `Allow: POST`; `POST /` → `405` with `Allow: GET`. Cases: err_not_found, err_api_wrong_method, err_page_wrong_method.
7. `POST /api` with `Content-Type: text/plain` → `415 Unsupported Media Type`, even with the right token. Cases: err_api_wrong_content_type_never_evaluated.
8. A request line that is not HTTP, `hello`, → `400 Bad Request`; `Transfer-Encoding: chunked` → `501 Not Implemented`. Cases: err_request_line_not_http, err_header_line_no_colon, err_content_length_not_a_number, err_transfer_encoding_chunked.
9. A `Content-Length` of `9000000` → `413 Content Too Large`, answered without reading a body; headers past 16 KiB → `431 Request Header Fields Too Large`. Generate the long case with a script and the Write tool, as U0's deep-nesting case was. Cases: err_body_too_large, err_headers_too_large, err_header_line_too_large, err_request_line_too_large.
10. A malformed protocol body in a well-formed request, `not json`, → `200 OK` with the U0 malformed-request answer as its body: HTTP succeeded, and the protocol answered. Cases: malformed_protocol_body_answered.
11. Header names in any case: `host:`, `x-splatcrab-token:` and `CONTENT-TYPE:` → `200 OK`. Cases: header_names_any_case.
12. `tests/ui_server.rs` (an integration test, not a golden case): over a real socket, an eval round trip, a 403 without the token, a 403 for a foreign `Host`, and `GET /` returning `200` with the page's `Content-Security-Policy`
13. Unit tests in `src/http.rs`: every status above, header order, `Content-Length` equal to the body's byte count on every response, the static routes' types and bodies equal to the embedded files, and the size limits at and past their bounds
14. The REPL, script and `.proto` cases pass unchanged

## Status

Done (2026-09-28)
