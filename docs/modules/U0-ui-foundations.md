# U0 — UI foundations

## Goal

The evaluation protocol the interface will speak, as a program on stdin and
stdout: `splatcrab --protocol` reads one JSON request per line and writes one
JSON response per line, against one interpreter session that persists across
requests. No socket, no HTTP, no browser. The existing golden harness covers
the whole of it, so cycle U1 is left as a thin transport over something
already proven.

After this module a user can do things that were not possible before; every
bullet in Scope must be demonstrable by at least one acceptance test below.

## Scope

- **`splatcrab --protocol`.** Reads requests from stdin until end of input
  and answers each on stdout before reading the next. It exits 0 at end of
  input, whatever the requests did: a session is not a script, and an error
  in an evaluation is an answer, not a failure of the program. Blank lines
  and a trailing `\r` are ignored. Nothing but responses is written to
  stdout, and nothing at all to stderr
- **Framing: JSON Lines.** A request is one JSON object on one line; a
  response is one JSON object on one line, with its keys in the fixed order
  this spec gives, so that golden output is deterministic. Strings are
  escaped as JSON requires: `\"`, `\\`, `\n`, `\r`, `\t`, and `\u00XX` for
  any other control character. Everything else, `×` included, is written as
  raw UTF-8
- **`src/json.rs`**, hand-written, no crate: a `Json` value (null, bool,
  number, string, array, object with its keys in order), a parser and a
  writer. The parser is depth-limited, as invariant 6 requires of anything
  that recurses on its input: a request nested past the limit is a malformed
  request, never a stack overflow. Cycle U1 reuses this module
- **`src/protocol.rs`**, the request loop as a library function taking any
  reader and writer, so unit tests can drive it without a process.
  `src/main.rs` only dispatches the flag to it
- **Request identity.** Every request may carry an `id`, a number or a
  string, which its response echoes as its first key. A request with none is
  answered with `"id":null`
- **`eval`**: `{"id":1,"op":"eval","code":"x = 1 + 2"}`. The code runs as a
  REPL entry does, possibly several lines, with its output captured rather
  than written. Success is `{"id":1,"ok":true,"out":"<output>"}`. An error is
  `{"id":1,"ok":false,"out":"<output before the error>","error":{"message":"<text>","line":<n or null>}}`,
  where `message` is the text the REPL prints after `Error: ` and `line` is
  the one-based line within `code`, or `null` when none is known. The session
  keeps every variable assigned before the error
- **`complete`**: `{"id":2,"op":"complete","code":"for k = 1:3"}` answers
  `{"id":2,"ok":true,"complete":false}`: whether `code` is a finished entry
  or is still inside an open bracket or block. This is the REPL's
  `needs_more`, moved out of `src/main.rs` into `src/syntax.rs` as
  `syntax::is_complete` and used by both, so the terminal and the interface
  can never disagree about when an entry ends. `docs/ARCHITECTURE.md`'s "Add
  a statement" recipe already names it there
- **`workspace`**: `{"id":3,"op":"workspace"}` answers
  `{"id":3,"ok":true,"vars":[{"name":"s","size":[1,2],"class":"char"}, …]}`,
  one entry per variable, sorted by name. It is what U2's workspace pane
  shows
- **`completions`**: `{"id":4,"op":"completions","prefix":"di"}` answers
  `{"id":4,"ok":true,"items":["diag","disp"]}`: every variable and builtin
  whose name starts with `prefix`, sorted, without duplicates (a variable
  that shadows a builtin appears once). It is `env::completions` in a new
  `src/env.rs`, with the variable and registry halves; cycle 13's spec
  extends it with path files and builds terminal tab completion on it
- **Malformed requests are answers, not exits.** A line that is not JSON, is
  not an object, has no `op` or an unknown one, or lacks a field its `op`
  needs is answered `{"id":<id or null>,"ok":false,"error":{"message":"<text>","line":null}}`,
  and the loop goes on to the next line. The texts are SplatCrab's own and
  live in `src/error.rs`, like every other message: `Malformed request: <what
  is wrong>.` and `Unknown operation '<op>'.`
- **A `.proto` golden case kind.** Like a `.repl` case, the harness spawns the
  binary with `--protocol` and types the file, less its `% covers:` line, on
  stdin; `.out`, `.err` and the exit-code rule are unchanged. Documented in
  `docs/TESTING.md` and at the top of `tests/golden.rs`
- The REPL behaves exactly as before. Its cases are the regression check for
  the move of `needs_more`

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.
- Sockets, HTTP and the page: cycle U1.
- Interrupting a running evaluation, and streaming output while it runs. An
  `eval` answers once, when it finishes.
- Values in `workspace` beyond name, size and class. A value preview needs
  display rules for a pane, which is U2's to decide.
- `exit` and `quit` inside `eval`. They are REPL-only lines today and become
  statements in cycle 13; until then `exit` in an `eval` is the ordinary
  unrecognized-name error.
- Files, the working directory and path completions: cycle 13.

## Design notes

Filled in as decisions are made. Start from the
"Key designs to preserve" section of docs/ARCHITECTURE.md and record here:
which files and types changed, which invariants were preserved (column-major
storage, 1-based to 0-based conversion at the index boundary, the `end` stack,
name resolution order, the output sink), and any deviation from this spec
that was accepted deliberately.

Settled at planning, so that the implementation does not have to guess:

- JSON rather than a format of our own, because the browser in U1 sends and
  reads JSON natively and U1 needs a writer regardless.
- One line per message rather than a length prefix, so that a `.proto` case
  is readable and writable by hand. A newline inside `code` is the JSON
  escape `\n`, never a raw newline.
- Output is captured by swapping `Interp.out` for a buffer for the length of
  one `eval`, which is what that sink exists for. `print!` stays forbidden
  outside `src/main.rs`; the protocol writes to the writer it is handed.
- Key order is part of the protocol: `id`, `ok`, then the operation's keys in
  the order written above, then `error` last.

Recorded during implementation.

**Files and types.** New: `src/json.rs` (`Json { Null, Bool, Number(f64),
String, Array, Object(Vec<(String, Json)>) }`, `json::parse`,
`Json::write`/`Display`, `json::write_string`, `ParseError { Syntax(usize),
TooDeep }`, `json::MAX_DEPTH`), `src/syntax.rs` (`is_complete`), `src/env.rs`
(`completions`) and `src/protocol.rs` (`serve`, `serve_with`, `respond`).
Changed: `src/main.rs` dispatches `--protocol` and calls
`syntax::is_complete` where it had `needs_more`; `src/lib.rs` exposes the
four modules; `src/interp.rs` gains the read-only accessor
`Interp::builtins()` for `env::completions`; `src/error.rs` gains the
protocol's constructors and scans the four new files for inline message text;
`tests/golden.rs` gains the `.proto` kind; `.gitattributes` forces LF for
`.proto`.

**Invariants.** Nothing in the evaluator changed, so column-major storage, the
one-based conversion in `eval_index_args`, the `end` stack and the name
resolution order are untouched. The output sink is the point of the design:
`protocol::serve` builds its `Interp` over `io::sink()`, and `eval` swaps
`Interp.out` for a shared buffer for the length of one `Interp::run` and puts
the sink back, so nothing the interpreter prints reaches the writer except
inside a response. Invariant 6: the JSON parser recurses once per array or
object level and counts levels against `json::MAX_DEPTH`.

**The depth limit is 128.** A document nested exactly 128 arrays or objects
deep parses and one level more is `ParseError::TooDeep`, checked before
recursing, so a line of 100,000 `[` is refused at level 129 without ever
approaching the stack. 128 is the default of the common Rust and Python JSON
parsers; the protocol's requests are flat objects, so it is far above any real
request, and far enough below `parser::MAX_DEPTH`'s 10,000 that the unit tests
are safe on a test thread's small stack. The writer recurses too, but only
over values built here or parsed within the limit.

**Choices where the spec was silent.**

- The protocol's messages, settled in testing and all constructed in
  `src/error.rs`. Each follows the spec's `Malformed request: <what is
  wrong>.` form, names the one thing wrong, and is pinned by a golden case:

  | Message | When | Case |
  |---|---|---|
  | `Malformed request: not valid JSON.` | any syntax error, trailing text after the value included | `err_malformed_not_json`, `err_malformed_bad_id` |
  | `Malformed request: nested too deeply.` | arrays or objects nested past `json::MAX_DEPTH` | `err_deep_nesting` |
  | `Malformed request: not a JSON object.` | valid JSON that is not an object | `err_malformed_not_object` |
  | `Malformed request: 'id' must be a number or a string.` | an `id` that is an array, an object or a boolean | `err_malformed_bad_id` |
  | `Malformed request: no '<field>' field.` | `op`, or the `code` or `prefix` its operation needs, absent | `err_malformed_no_op`, `err_malformed_eval_no_code`, `err_malformed_field_not_string` |
  | `Malformed request: '<field>' must be a string.` | that field present but not a string, `null` included | `err_malformed_field_not_string` |
  | `Unknown operation '<op>'.` | an `op` string that names no operation | `err_unknown_operation` |

  The `no '<field>' field` and `nested too deeply` wordings were adopted
  from the golden cases, written alongside the code, so that neither side's
  guess was forced on the other. The limit's number is in these notes and
  `docs/ARCHITECTURE.md`, not in the message.
- An `eval` error's `message` is the interpreter's own text, the one script
  mode prints after `Error: Line N: `: an unclosed `for k = 1:3` is
  `expected 'end' but found end of input` on line 1 (script mode reports the
  same text, on line 2 when the file ends in a newline), and `a = 7; b =
  a(3)` is `Index exceeds the number of array elements. Index must not exceed
  1.` on line 1, as script mode prints it.
- `handbook_protocol_example` sends the six requests of the protocol example
  in `docs/HANDBOOK.md` and expects its six response lines, so the handbook
  example, which the handbook runner cannot run, is checked all the same.
- Checks run in this order: JSON, object, `id`, `op`, the operation's fields.
  So an `id` is echoed in every refusal after the `id` check, and a bad `id`
  (an array, an object, a boolean) is answered with `"id":null`.
- `"id":null` is treated as no id. Unknown extra fields are ignored. A
  duplicated key reads its last value, as `JSON.parse` does, and a written
  object keeps every pair in order.
- An id is echoed by value: a number is written in Rust's shortest round-trip
  form, so `1` and `1.0` both come back as `1`, `2.5` as `2.5`. Every number
  the protocol writes (ids, sizes, lines) is an integer with no decimal point.
  A non-finite number, which JSON cannot express, would be written `null`.
- A control character is escaped as `\u00xx` with lower-case hex, as
  `JSON.stringify` writes it, so `clc`'s escape is `\u001b`.
- A `\u` escape of a lone surrogate, which has no Rust `char`, decodes to
  U+FFFD rather than refusing the request, the same leniency script mode
  gives an undecodable byte. Input lines are read as bytes and decoded
  lossily for the same reason, so invalid UTF-8 is never an I/O error.
- A line of nothing but whitespace is skipped like a blank one.
- `completions` sorts by byte order, as `who` does, so capitals come first
  (`Inf`, `NaN`, then lower-case names). An empty prefix lists every name.
- `syntax::is_complete` of text that does not lex is `true`, which is what
  `needs_more` returning `false` meant: running it reports the error.
- `serve` returns `io::Result<()>`. `Ok` at end of input exits 0; an `Err`
  means reading stdin or writing stdout failed, the transport rather than a
  request, and `main.rs` exits 1 without writing to stderr, keeping the
  promise that the protocol writes nothing there.
- `eval`'s `line` is the `MError`'s line as `Interp::run` reports it, which
  is already one-based within the submitted code; a parse error has one too,
  so `for k = 1:3` is `expected 'end' but found end of input` on line 1.

- "Any other control character" in the escaping bullet means a character
  below U+0020, as JSON defines it and as `JSON.stringify` writes it. DEL
  (U+007F) and U+0080 to U+009F are written raw, and a unit test pins DEL.
- The trailing-`
` rule is pinned by a unit test only: `.gitattributes`
  forces LF on `.proto` files, so a golden case cannot carry a CR.
- `serve` reads a line with no length limit, which is harmless on stdio.
  Cycle U1 must cap the line length before a socket goes in front of it.
- Added at review: the harness asserts an empty stderr for every `.proto`
  case without an `.err`, which Scope's "nothing at all to stderr" and item 2
  require. Two case comments had said the harness could not; it can, and
  was checked by a stray `eprintln!` turning all 21 cases red.

**Deviations.** None from Scope.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/U0-ui-foundations/`, as a `.proto` case unless it says
otherwise. Each request below is one line of the case's stdin, and each
response one line of its `.out`. Expected output comes from this spec's rules
and from outputs existing cases already pin: the display text inside `out` is
exactly what the same code prints in script mode.

1. `{"id":1,"op":"eval","code":"x = 1 + 2"}` then `{"id":2,"op":"eval","code":"disp(x * 2)"}` → `{"id":1,"ok":true,"out":"x =\n\n     3\n\n"}` then `{"id":2,"ok":true,"out":"     6\n"}`: display is captured, and the session persists. Cases: eval_display_and_session.
2. `{"id":3,"op":"eval","code":"disp(1)\ny = nosuchname"}` → `{"id":3,"ok":false,"out":"     1\n","error":{"message":"Unrecognized function or variable 'nosuchname'.","line":2}}`; and a following `{"id":4,"op":"eval","code":"z = 5;"}` → `{"id":4,"ok":true,"out":""}`, since an error does not end the session. The process exits 0 and writes nothing to stderr. Cases: err_eval_answer_session_survives.
3. `{"id":5,"op":"eval","code":"a = 7; b = a(3)"}` then `{"id":6,"op":"eval","code":"disp(a)"}` → the first fails with `"line":1`, and the second prints `     7`: variables assigned before an error survive it. Cases: err_eval_keeps_earlier_assignments.
4. `complete` on `for k = 1:3` → `false`; on `for k = 1:3\ndisp(k)\nend` → `true`; on `x = [1 2` → `false`; on `c{end}` → `true`; on `x = 1` → `true`. Cases: complete_open_and_closed.
5. After `{"op":"eval","code":"x = 1:3; s = 'ab'; t = true;"}`, `{"id":7,"op":"workspace"}` → `{"id":7,"ok":true,"vars":[{"name":"s","size":[1,2],"class":"char"},{"name":"t","size":[1,1],"class":"logical"},{"name":"x","size":[1,3],"class":"double"}]}`; the eval's own response has `"id":null`. Cases: workspace_sorted_with_class.
6. `{"id":8,"op":"completions","prefix":"dis"}` → `{"id":8,"ok":true,"items":["disp"]}`; after `display_count = 1;` the same request → `["disp","display_count"]`; and after `disp = 3;`, `{"id":9,"op":"completions","prefix":"disp"}` still lists `disp` once. Cases: completions_builtins_and_variables, completions_shadow_listed_once.
7. Malformed lines, each followed by a good request that still succeeds: `not json`; `[1, 2]`; `{"id":10}`; `{"id":11,"op":"fly"}` → `"message":"Unknown operation 'fly'."`; `{"id":12,"op":"eval"}` (no `code`). Every malformed answer has `"ok":false` and `"line":null`, and echoes the `id` when it could be read. Cases: err_malformed_not_json, err_malformed_not_object, err_malformed_no_op, err_unknown_operation, err_malformed_eval_no_code, err_malformed_field_not_string, err_malformed_bad_id.
8. Escaping: `{"id":13,"op":"eval","code":"disp('a\"b\\c')"}` → `"out":"a\"b\\c\n"`; `{"id":14,"op":"eval","code":"fprintf('x\\ty\\n')"}` → `"out":"x\ty\n"`; `{"id":15,"op":"eval","code":"v = true(1, 2)"}` → an `out` holding the raw `1×2 logical array` header, not a `×` escape. Cases: escape_quote_and_backslash, escape_tab, raw_utf8_times.
9. `{"id":"abc","op":"workspace"}` → a response beginning `{"id":"abc","ok":true`. Cases: string_id_echoed.
10. A request nested 100000 levels deep (`[[[[…`) is answered as a malformed request and exits 0, not 134. Generate its stdin with a unit test, or write the case's `.proto` with a script and the Write tool: it is one long line. Cases: err_deep_nesting.
11. An `eval` whose code has an unclosed block, `{"id":16,"op":"eval","code":"for k = 1:3"}`, is an error answer, not a hang waiting for more input: the protocol runs exactly the code it is sent, and a client asks `complete` first. Cases: err_eval_unclosed_block.
12. The REPL cases in `01b-error-reporting/`, `01e-display-and-parser/` and `03-indexing-forms/` pass unchanged: the move of `needs_more` into `syntax::is_complete` changes no behaviour
13. Unit tests in `src/json.rs`: round-trips of every value kind, the escapes above, surrogate-pair `\u` escapes decoding to one character, rejection of trailing garbage, and the depth limit

## Status

Done (2026-09-28)
