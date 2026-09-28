# Architecture

```
 .m source ──► lexer.rs ──► parser.rs ──► interp.rs ──► value.rs
              tokens       Stmt / Expr   tree-walking   column-major
              + lines      + lines       evaluator      f64 matrices
                                              │         + a class tag
                                              │
                                         builtins/
                                         the library, behind a registry

                            error.rs: MError, and every message text

 JSON line ──► protocol.rs ──► json.rs      parse the request, write the reply
                   │
                   ├──► interp.rs           eval, output captured
                   ├──► syntax.rs           complete: is the entry finished?
                   └──► env.rs              completions: variables + builtins

 browser ──► server.rs ──► http.rs ──► protocol::respond
 127.0.0.1   accept, read   limits, Host, Origin, token, routes
 only        one request,   │
             write, close   └──► src/ui/    index.html, app.js, app.css,
                                            embedded with include_str!
 stdin ────► http::serve_stdio (--http-stdio): the same http::handle, no socket
```

`src/lib.rs` exposes the twelve modules: the six of the language (`lexer`,
`parser`, `interp`, `value`, `builtins`, `error`), the four of the
evaluation protocol that cycle U0 added (`json`, `syntax`, `env`,
`protocol`) and the two of the UI server that cycle U1 added (`http`,
`server`). `src/ui/` holds the page's three files, which `http.rs` embeds.
`src/main.rs` is the CLI and REPL and is the only file allowed to use
`print!`. It runs everything, `--protocol` and `--ui` included, on a thread
with a 256 MB stack, because Windows gives the main thread 1 MB and the
parser and the evaluator each recurse once per nesting level.

## The modules

**`lexer.rs`** turns source into a `Lexed`: a `Vec<Token>` and a parallel
`Vec<u32>` of the line each token came from. `lex` returns the tokens alone
for callers with no use for the lines. Two MATLAB quirks live here rather than
in the parser, because they need character-level context:

- Inside `[ ]`, whitespace separates elements. `[1 -2]` is two elements and
  `[1 - 2]` is one. The lexer tracks a stack of open delimiters and inserts a
  `Comma` when whitespace sits between something that ends a value and
  something that starts one. A newline inside brackets becomes `Semi`.
- A quote is a transpose after a value and a string delimiter otherwise. The
  `ends_value` helper decides which.

A `...` continuation is a gap between tokens exactly as whitespace is, and
goes through the same separator check, which is what makes `[1 ...` newline
`-2]` two elements. The number lexer's "do not swallow the dot" exclusion list
covers `* / \ ^ ' .`: the backslash keeps `2.\x` from meaning `2 \ x`, and the
dot keeps `a = 1...` from lexing as `1.` plus a stray `..`.

Since cycle 04 two more things live here, for the same reason:

- **Block comments.** A `%{` alone on its line (surrounding whitespace
  allowed) opens one and a `%}` alone on its line closes it. They nest, and
  the lexer counts them rather than recursing. An unterminated one runs to the
  end of the source, and `Lexed::open_comment` says so, which is how
  `syntax::is_complete` knows the entry is unfinished.
- **Command syntax.** A name that starts a statement, is not a variable, and
  is followed by whitespace and then a word that is not an operator followed
  by whitespace, is a command: the lexer reads the words (quotes group them;
  a newline, `,`, `;` or `%` outside quotes ends them) and emits the call they
  mean, `name('w1', 'w2')`, so the parser never sees command syntax.
  "Is a variable" is judged when the source is lexed, from the workspace
  `scan_known` is told about and the names the source has assigned so far
  (`x = `, `x(2) = `, `for x = `, `[a, x] = `, `catch x`), as MATLAB judges it
  in a file. `scan` is `scan_known` with no workspace. A statement starts
  after a newline, `;` or `,` with no bracket open, and after `else`, `try`
  and `otherwise`.

The brace of a `case {...}` list goes on the delimiter stack as `C`, which
the whitespace rule treats as a bracket, so `case {2 3}` is two values.

Since cycle 03 the lexer also has `{`, `}`, a lone `.` and `@`. The field dot
is whatever dot is left once a number's decimal point, the five dotted
operators (`.*`, `./`, `.\`, `.^`, `.'`) and a `...` continuation have been
recognised, so none of those changed. A `}` ends a value, so `c{1}'` is a
transpose. Braces go on the delimiter stack like brackets, so that inside
`[c{1 -2} 3]` the whitespace rule applies to the bracket and not within the
braces; inside brackets a `{` or an `@` after a space starts a new element.

**`parser.rs`** is recursive descent with MATLAB's precedence, loosest first:
`||`, `&&`, `|`, `&`, comparison, `:`, `+ -`, `* / \ .* ./ .\`, unary `- ~`,
`^ .^`, transpose. `end` and a bare `:` are only accepted inside an index
argument list, tracked by the `in_index` counter. `Expr`, `Stmt`, `BinOp` and
`Token` derive `PartialEq` so tests can compare trees directly. A block is a
`Vec<Located>`, where `Located` is a `Stmt` plus the line it starts on; the
line sits on a wrapper so the tree shape stays comparable on its own. An `if`
arm is an `IfArm`, which carries its condition's own line for the same reason:
an error in an `elseif` condition must name the `elseif`.

A name followed by `(...)`, `{...}`, `.field` or `.(expr)` is
`Expr::Access(name, Vec<Access>)`, the whole chain in the order written
(`c{1}(2).b` is one node with three links); a bare name is `Expr::Ident`, and
`name(args)` is a one-link chain whose first `(...)` is indexing or a call.
An assignment target is an `LValue`, a name and a chain, so `x = v`,
`x(2) = v` and `s.a = v` are all `Stmt::Assign(LValue, Expr, bool)`.
`[a, ~, c] = rhs` is `Stmt::MultiAssign(Vec<Option<LValue>>, Expr, bool)`,
with `None` for a `~`: `try_targets` reads a bracket as a target list when it
holds only targets and placeholders and is followed by `=`, and otherwise
rewinds so that the bracket parses as the matrix literal it always was. A bare
`@`, and a `{` where a value should start, are parse errors until cycles 06
and 07.

Cycle 04 added `Stmt::Switch(subject, Vec<CaseArm>, otherwise)` and
`Stmt::Try(body, Option<String>, handler)`. A `CaseArm` carries its values
(one for `case x`, several for `case {a, b}`), its own line, as an `IfArm`
does, and its body. The name after `catch` is the bound variable only when
it follows `catch` directly, with no comma or newline between.

Cycle 05 added `function` and `return`. `parse_program` returns a `Program`,
the statements and then the `Function`s defined after them (name, outputs,
parameters with `~` kept in place, body, line); a statement after a function
is MATLAB's "Function definitions in a script must appear at the end of the
file.", and a `function` inside a block is "Function definitions are not
supported in this context." A body is read as a block that stops at `end` or
at the next `function`, so a function file's functions may all go without
`end`. `Stmt::Return` is the one new statement. The lexer makes a function
line's outputs and parameters variables of the body for command syntax, and
forgets the names assigned before it.

`Token` also has a `Display` form, which is what every parse message renders
the offending token through; its `Debug` is the Rust variant name and used to
reach the user as `unexpected Semi in expression`. The parser counts nesting
in `depth` against `MAX_DEPTH` and refuses anything deeper, both when it
recurses (`((x))`, `f(f(x))`, a nested block) and when a left-folding loop
deepens the tree without recursing (`1+1+…+1`). `Interp` counts the same way
against the same constant, so a program the parser accepts is one the
evaluator can walk; see invariant 6.

**`error.rs`** holds `MError { msg, line }` with its identifier and, since
cycle 05, its `stack` of `StackEntry { name, line }` boxed together behind
`identifier()` and `stack()` (an `MError` rides in every `R<Value>`, so its
size is paid in every frame of a deep recursion), the `R<T>` alias every fallible
path returns, a `bail!` macro, and a constructor for every message the
interpreter can raise. Nothing else in the crate spells a message out; a unit
test scans the other files for an `Err(`, `bail!(` or `ok_or_else` handed a
literal or a `format!` and fails if it finds one. `MError::at` records a line
only if none is known yet, so the innermost statement wins. The
`identifier` is the one `error('id:x', fmt, ...)` attached, and empty for
every error the interpreter raises itself.

**`value.rs`** holds `Matrix`, `Class` and `Value`. Matrices are
**column-major**, the same as MATLAB: element `(r, c)` lives at
`data[c * rows + r]`. This is not an implementation detail. It is what makes
`A(:)`, `reshape`, and linear indexing produce MATLAB's answers, and every new
operation must respect it. Every `Matrix` carries a `class` tag, `Double`,
`Logical` or `Char`, over the same `f64` storage; a char element is one UTF-16
code unit. `value.rs` also owns the display: `format` for the numeric body,
`disp_text` for `disp`, and `display_body` for the class headers of a named
display. Since cycle 04 `Value` has a second variant, `Exception`, the
`MException` a `catch` binds; `Value::mat` and `Value::into_mat` return an
`R`, refusing it with `This operation is not supported for a value of class
'MException'.`

**`interp.rs`** walks the tree. It resolves `name(args)` as indexing when
`name` is a variable and as a call otherwise, and grows arrays on indexed
assignment. Since cycle 05 it holds a stack of `Frame`s and calls user
functions: `call_function` is invariant 4, `call_user` runs a function in a
frame of its own, `run_script` runs a script file in the caller's, and
`find_file` and `load` look files up on the path (`Interp::cwd` first, then
the `addpath` folders) through a lookup cache and a file cache that a
generation counter keeps honest. `run` runs a script, local functions
allowed; `run_command` is the REPL's and the protocol's, and refuses a
definition. It no longer knows what any individual builtin does.
Reading, assignment and deletion share one index pipeline since cycle 03:
`eval_index_args` turns the subscripts into zero-based `Sel`s (a logical
subscript becomes the positions `find` would give), `resolve_read`,
`resolve_write` or `resolve_delete` judges them against the array's shape
without changing anything, and `gather`, `scatter` or the deletion carries the
plan out. Because the plan is complete before anything moves, an indexed
assignment validates every subscript, the class conversion, the growth and
the element count first and then changes the variable where it is stored,
never cloning it; growth along the last dimension is a `Vec` resize, which is
amortised.

`try` runs its body and, on an error, puts the nesting counters and
`end_stack` back to what they were at the `try` (a failed `deepen` does not
undo itself), records the message for `lasterr`, binds the error as a
`Value::Exception` if `catch` named a variable, and runs the handler. A
`break` or `continue` in the body is a `Flow` like any other and passes
through. `switch` evaluates its case values in order, only until one
matches.

**`builtins/`** is the library: `mod.rs` holds the registry, `args.rs` the
argument helpers, and `core.rs`, `math.rs` and `linalg.rs` the builtins
themselves. Every one has the same shape,
`fn(&mut Interp, &[Value], usize) -> R<Vec<Value>>`, where the `usize` is
`nargout` and an empty `Vec` means the builtin produced no value.

**`syntax.rs`** answers questions about source text short of parsing it.
`is_complete` says whether an entry typed line by line has ended, by counting
brackets and block openers against their closers over the token stream; the
REPL asks it after every line and the protocol's `complete` asks it on
request, so the two can never disagree. It was `needs_more` in `main.rs`
until cycle U0. Since cycle 04 it counts `switch` and `try` as openers
beside `if`, `for` and `while`, and an open `%{` block comment as
unfinished; since cycle 05 `function` too, so a definition is read whole
before it is refused.

**`env.rs`** is the environment as seen from outside the evaluator.
`completions(prefix, &vars, &registry)` lists every variable and builtin
starting with `prefix`, sorted by byte order and deduplicated; cycle 13 adds
path files to it and builds the terminal's tab completion on it.
`Interp::builtins` hands out the registry read-only for it.

**`json.rs`** is hand-written JSON: a `Json` value whose objects keep their
keys in written order (a `Vec` of pairs, not a map), a parser and a writer.
The parser counts array and object levels against `json::MAX_DEPTH`, 128, and
refuses anything deeper, for invariant 6. The writer escapes exactly `"`,
`\`, newline, carriage return and tab by name and other control characters as
`\u00xx`, and writes everything else as raw UTF-8. Cycle U1 reuses it.

**`protocol.rs`** is `splatcrab --protocol`: `serve(reader, writer)` reads
one JSON request per line and writes one JSON response per line against one
`Interp`, flushing after each. `eval` swaps `Interp.out` for a buffer for the
length of the call and restores it, which is what the sink is for; the
interpreter is built over a sink otherwise, so nothing but responses reaches
the writer. Since cycle 04 `eval` points `Interp.err` at the same buffer, so
a warning lands in `out` where it was raised and nothing reaches stderr. A
failed evaluation and a malformed request are both answers; the loop ends
only at end of input. The message texts live in `error.rs`.

**`http.rs`** is the UI server's HTTP, as a pure function:
`handle(request_bytes, &mut Interp, &Config) -> Vec<u8>`, with `Config`
holding the port and the token. It parses the head (request line, headers
with case-insensitive names, CRLF or bare LF), refuses what it must with
`400`, `403`, `404`, `405`, `413`, `415`, `431` or `501`, serves the three
embedded files, and hands the body of a `POST /api` that passed every check
to `protocol::respond`. Responses are built in one place, so the header
order and a `Content-Length` equal to the body's length hold for all of
them; the status texts are a table in `error.rs`. `read_request` frames one
request off any `BufRead` under the head and body caps, shared by the socket
and by `serve_stdio`, the `--http-stdio` loop the golden cases drive.

**`server.rs`** is the socket around it: `bind` to `127.0.0.1`, the session
token, the browser launch, and `serve`, which accepts one connection at a
time on the interpreter thread, reads one request under a 10-second deadline
for the whole of it, so a client trickling a byte at a time cannot hold the
server, writes the answer and closes. It holds no policy: every check is in
`http.rs`, where a unit test can reach it.

**`src/ui/`** is the page: `index.html`, `app.js` and `app.css`, a command
window with no framework and no external resource, embedded with
`include_str!` so the binary is the whole program.

**`main.rs`** is the CLI. On Windows it first switches the console's output
code page to UTF-8 with `SetConsoleOutputCP(65001)`, declared as a raw
`extern "system"` function under `#[cfg(windows)]`, because the crate takes no
dependencies; everything the interpreter writes is UTF-8 already.

## Invariants

These hold everywhere. Breaking one is a bug even if the tests pass.

1. **Column-major storage.** See above.
2. **One-based to zero-based conversion happens at exactly one boundary**,
   in `eval_index_args` (through its helpers `index_positions` and
   `mask_positions`), for reading, assignment and deletion alike. Everything
   downstream of it, the `resolve_*` functions included, is zero-based;
   everything in user-facing error messages is one-based.
3. **`end` is resolved through the running frame's `end_stack`**, pushed per
   index argument with the size of the dimension being indexed. Since cycle
   05 the stack lives in the `Frame`, so a call starts with an empty one:
   `x(f(end))` binds `end` to `x`, and nothing `f` indexes can see it.
4. **Name resolution order is variable, then the running file's local
   functions, then the script's local functions, then a file on the path,
   then a builtin** (`Interp::call_function`). A user variable shadows every
   function, as in MATLAB: after `sum = 3`, `sum(1)` indexes the variable.
   A file on the path, the current folder first, shadows a builtin of its
   name.
5. **All interpreter output goes through `Interp::emit`.** No `print!` outside
   `src/main.rs`. Tests swap `Interp.out` for a buffer to capture output.
   Diagnostics a program goes on after, `warning` since cycle 04 and cycle
   11's `fprintf(2, ...)`, go through `Interp::emit_err` to the second sink,
   `Interp.err`, which flushes `out` first so the two stay in order. `main.rs`
   makes it stderr for a script and the REPL; the protocol, and so `--ui`,
   points it at the same capture as `out`.
6. **Errors are values, not panics.** Every fallible path returns `R<T>`,
   which is `Result<T, MError>`. A panic is a bug; the REPL must survive any
   bad input. Restored in cycle 01e, which closed the last input that could
   abort the process. Holding it is what the depth limit of
   `parser::MAX_DEPTH` is for: the parser and the evaluator both recurse once
   per nesting level, and recursion bounded only by the stack cannot return a
   value when it runs out. The JSON parser of the protocol recurses on its
   input too, and has its own, smaller bound, `json::MAX_DEPTH`. The UI
   server reads a request under two caps, `http::MAX_HEAD` (16 KiB) and
   `http::MAX_BODY` (8 MiB), judged before the bytes are buffered, so no
   client can make it allocate without bound. Anything
   that computes a result shape from its
   operands' shapes goes through `args::check_shape` for the same reason; see
   the recipe below.

## Recipes

### Add a builtin

1. Write the function in the right file under `src/builtins/`: `core.rs` for
   constants, constructors, shape queries, output, the workspace and timing;
   `math.rs` for element-wise and reducing numerics; `linalg.rs` for linear
   algebra, rearrangement, search and sort. The signature is
   `fn(&mut Interp, &[Value], usize) -> R<Vec<Value>>`; return `one_mat(m)`
   for a numeric value and `none()` for a builtin that produces none.
   `one_mat` makes its result a double whatever `m`'s class, which is MATLAB's
   rule for every numeric builtin; a builtin that decides its own class (a
   rearrangement keeping its argument's, a predicate returning a logical)
   returns `one_as(m)`, and a char result `one(Value::str(..))`.
2. Add one line to that file's `register`, with a one-line help string. The
   table is `#[rustfmt::skip]`ed so it stays one line per name.
3. Bump `EXPECTED` in the registry test in `src/builtins/mod.rs`.
4. Add a golden case exercising it, and an `err_*` case for each new error.
5. Add a row to `docs/FEATURES.md` and the name to `README.md`.

Use the helpers in `src/builtins/args.rs` for argument access; they produce
the MATLAB-style messages. `need` and `at_most` bound the argument count,
`mat`, `scalar` and `string` fetch one, and `option` returns a char option
such as `'descend'` for the caller to match; both read a char argument's
UTF-16 code units back into a Rust `String`. `dim` reads a dimension argument
(a positive integer, never a char), and `dim_or_all` also accepts `'all'`,
for the reductions that take it.

A shape the user asks for goes through the size helpers and stays `f64` until
it is judged, so that an oversized request is named as asked. `shape` reads a
constructor's sizes: none (1x1), a scalar `n` (n x n), a row size vector, or
two or more scalars, with trailing sizes of `1` dropped and any other third
size the N-D error. `size_list` and `trailing_ones` are its lower layers, for
a builtin such as `reshape` that takes a `[]` placeholder or has no n-by-n
rule. `size_arg` and `size_value` read one size as an `f64`, where a negative
size is `0`. `check_shape(rows, cols)` is the only sanctioned way to turn a
requested shape into allocation lengths. Indexed growth uses it too since
cycle 03, with the grown size kept as an `f64` until it is judged; the
`usize`-only `check_size` that growth used before went with it.

A shape the user never spells out goes through `check_shape` too. Since cycle
01d, any operation whose result shape is computed from its operands' shapes
calls it before allocating: `Matrix::try_zip` (and so `zip`), `Matrix::matmul`,
the two-subscript branch of `resolve_read` and `math::reduce`. The operands can
be tiny and the result enormous — `ones(1e5, 1) + ones(1, 1e5)` asks for 1e10
elements from 2e5 — so "the operands fit, therefore the result fits" is never
true. A new operation of that kind belongs on the same list.

### Add a statement

1. `lexer.rs`: add the keyword to the `match` in the identifier branch so it
   lexes as a `Token` rather than an `Ident`.
2. `parser.rs`: add the `Stmt` variant and a branch in `parse_stmt`.
3. `interp.rs`: add the arm in `exec`, returning the right `Flow`.
4. `syntax.rs`: if the statement opens a block, teach `is_complete` to count
   its keyword beside `if`, `for` and `while`, so that the REPL keeps
   reading lines and the protocol's `complete` answers `false` until the
   block is closed. This lived in `main.rs` as
   `needs_more` until cycle U0 moved it out; look there, not in the binary.

### Add a value type

`Value` has one variant, `Mat`, since cycle 02 removed `Str`: text is a
`Matrix` whose `class` is `Char`. A new array class follows the rule cycle 02
set: class is a property of the array, storage stays numeric. `Matrix` has a
`class` tag rather than being generic, because MATLAB's own semantics work
that way (`'a' + 1` is `98`, `true + true` is `2`) and because every numeric
kernel then keeps compiling untouched. Only the constructors of results decide
the class: `Matrix::new` and every constructor built on it make a double,
`with_class` retags a result, and `to_class` converts one (a logical refuses a
`NaN`; a char rounds and clamps to a code unit). A new class needs a
`Class` variant, its name, its display in `display_body`, and its row in the
propagation rules in `interp.rs` (`binary`, `concat_class`, `assign_index`).

A container is a different kind of value, not a class of array. Cells and
structs (cycle 07) become new `Value` variants beside `Mat`, as the
`MException` of cycle 04 already is: `Value::Exception` holds the caught
`MError` whole, so `rethrow` raises it unchanged, and it answers
`class_name`, `dims`, `display_body` and `disp_text` itself. Every array
operation reaches a matrix through `Value::mat` or `Value::into_mat`, which
return an `R` and refuse any other variant, so a new variant is refused
everywhere by default and each operation that should accept it says so; a
`match` on `Value` makes the compiler list the sites that must decide.

## Key designs to preserve

These are decided and should not be re-litigated inside a cycle. The full
rationale is in the module specs under `docs/modules/`.

**Errors (cycle 01b, in place).** `MError { msg, line }` in `error.rs`, with
every message text defined there and nowhere else. Script mode prints
`Error: Line N: <msg>`; the REPL prints `Error: <msg>`, since a REPL entry is
one line. A statement's line is attached in `exec_block` by `MError::at`,
which keeps the first line it is given, so an error inside a `for` body
reports the body's line. Cycle 04 added the first further field,
`identifier`, and the `MException` that `catch e` binds is the `MError`
itself (see "Add a value type"). Cycle 05 added the stack, the same way: an
error leaving a user function or a path script passes `MError::leaving`,
which moves its line into a new `StackEntry` and clears it, so the calling
statement records its own. At the top the line is therefore always a line of
the code that was run, which is what the protocol's `line` means, and the
trace, `MError::trace`, is one `  in <fn> (line N)` per frame, innermost
first, which `main.rs` prints to stderr after the message, in script mode
and at the REPL alike. The protocol does not send it; `e.stack` is cycle
07's.

**Registry (cycle 01, in place).** `BuiltinFn = fn(&mut Interp, &[Value], usize) -> R<Vec<Value>>`,
where the `usize` is `nargout`. An empty `Vec` means the builtin produced no
value: legal at statement level, "Too many output arguments." in an expression.
Copy the function pointer out of the map before calling it, or the borrow
checker will object to `&self` and `&mut self` at once. `Stmt::Expr` asks for
0 values, `eval` asks for 1, and since cycle 03 `Stmt::MultiAssign` asks for
one per target, `~` included. A builtin returns as many values as it has, up
to `nargout`; the caller, not the builtin, turns too few into "Too many output
arguments.", so a builtin with one output needs no change to be asked for two.
`max`, `min`, `sort`, `size` and `find` give more than one.

**Classes (cycle 02, in place).** `Class { Double, Logical, Char }` as a tag
on `Matrix`. Arithmetic yields `Double`; comparisons and logical operators
yield `Logical`; concatenation yields `Char` if any operand is `Char`, else
`Logical` if all are, else `Double`, with a 0x0 double left out of the vote.
Indexed assignment keeps the left-hand side's class. A char element is a
UTF-16 code unit. The Design notes of `docs/modules/02-classes-and-display.md`
have the full table and every display rule.

**Protocol (cycle U0, in place).** JSON Lines over one persistent `Interp`,
`protocol::serve(reader, writer)`, with `main.rs` only dispatching
`--protocol` to it. Response keys come in a fixed order: `id`, `ok`, the
operation's own keys, `error` last. Output is captured by swapping
`Interp.out`, never by a second output path. Every failure a request can meet
is an answer and the process exits 0 at end of input. Cycle U1 puts a socket
in front of `serve` and reuses `json.rs`; the Design notes of
`docs/modules/U0-ui-foundations.md` have the details.

**UI server (cycle U1, in place).** An endpoint that runs code on a loopback
port can be reached by any web page the user visits, so the security model
is the design, and every part of it is in `http.rs` except the binding:

- `server::bind` binds `127.0.0.1` only, never a wildcard address.
- A fresh 128-bit token per run travels in the URL's fragment, which a
  browser never sends to a server or puts in a `Referer`, and comes back in
  the `X-SplatCrab-Token` header on every `/api` call, compared in constant
  time. A custom header makes a cross-origin request need a CORS preflight,
  which this server never approves, and the token makes a guess useless.
- Every request's `Host` must be `127.0.0.1:<port>` or `localhost:<port>`,
  which defeats DNS rebinding, and an `Origin`, when present, must be this
  server's own. A refusal is `403`.
- The head is capped at 16 KiB and the body at 8 MiB, both judged before
  buffering.
- A refused request never reaches the interpreter: only the last arm of
  `handle` calls `protocol::respond`, after every check has passed.
- The page's `Content-Security-Policy: default-src 'self'; frame-ancestors
  'none'` forbids inline script, anything from another origin, and framing.

HTTP stays minimal: `Connection: close` on every response, no keep-alive, no
chunked bodies, no `Expect: 100-continue`. The same `handle` is driven from
stdin by `--http-stdio`, so the golden cases pin every byte without a socket,
and `tests/ui_server.rs` covers the socket itself. The Design notes of
`docs/modules/U1-ui-server.md` have the details.

**Frames (cycle 05, in place).** A stack of `Frame { vars, end_stack, unit,
func_name, nargin, nargout }` with `frames[0]` as the base workspace, never
popped; `Interp::vars()` is the running frame's. Moving `end_stack` into the
frame is what stops `end` leaking across a call. `unit` is the parsed file
the frame's code came from, whose local functions come first in resolution;
a path script swaps its own in while it runs in the caller's frame.
Resolution order is variable, then the running file's local functions, then
the script's local functions, then a file on the path, then a builtin. User
files shadow builtins, as in MATLAB. Files are found against `Interp::cwd`,
never `std::env`, and cached by path; a generation counter, bumped by
`addpath`, `rmpath` and every `run`, makes every cached lookup and file stale,
and a stale file is reused only if its modification time and length are
unchanged. At most 500 calls run at once (`MAX_RECURSION`), and every frame
shares the one nesting budget of `MAX_DEPTH`; the Design notes of
`docs/modules/05-functions-and-scoping.md` have the stack measurements.

**Containers (cycle 07).** `CellArray` and `StructArray` as separate types that
reuse index-resolution helpers factored out of `Matrix`, rather than making
`Matrix` generic. Writes go through a recursive `assign_chain` that creates the
right empty container when a path does not exist yet.

## Known deviations from MATLAB

Recorded as `% NOTE:` lines in the affected golden cases, and fixed in the
cycle named:

| Deviation | Fixed in |
|---|---|
| `det([1 2; 3 4])` prints `    -2`, where the spec records MATLAB's `   -2.0000`. Cycle 02 fixed the display half: a value a rounding error from an integer now prints with decimals. The value half remains: this interpreter's pivoted elimination lands exactly on `-2`, because the last product `3 * 0.66666666666666674` is a rounding tie that goes to the even `2`, so there is nothing for the display to show. MATLAB's `-2.0000` implies LAPACK returns `-2.0000000000000004`, an operation order not reproduced here | 08, which replaces `det` with a shared LU factorisation (verify first) |
| An error text says more than MATLAB's and keeps its own wording: the dimension mismatch names the operator and both shapes, where MATLAB says only `Arrays have incompatible sizes for this operation.` | by design; see the message-text policy in `docs/modules/01e-display-and-parser.md` |
| `who` and `whos` print the same typed table | Both produce byte-identical output. In MATLAB `who` is a bare list of names and `whos` is a table with size, bytes and class, so both deviate rather than only `who`, and neither has a bytes column | 13 |
| `norm` and `sort` accept vectors only | 08, 09 |
| Backslash solves square systems only, and errors instead of warning | 08 |
| A result that would be complex is a clean error; MATLAB returns the value | 10 |
| A char range and `diag` of a char return doubles: `'a':'c'` is `97 98 99` and `diag('abc')` is numeric, where MATLAB keeps char. Cycle 02's Scope named six rearrangements and these were not among them | 11 |
| Chained indexing `x(2:3)(2)` is read successively, as Octave does; MATLAB refuses it. `x()` is "Only 1-D and 2-D indexing is supported." where MATLAB returns `x`. Both recorded in cycle 03's Design notes | later |
| Indexing into or growing a second page, `A(:, :, 2) = 5` or `A(:, :, [1 1])`, is "N-D arrays are not supported."; MATLAB builds the N-D array. Cycle 03 accepted it | later, with N-D arrays |
| An `MException` is minimal: `message`, `identifier` and `class`, with no `stack`, `cause` or `Correction`, and its display is SplatCrab's one line `  MException (id): msg` rather than MATLAB's property listing. An error the interpreter raises itself has an empty identifier, where MATLAB's carry one such as `MATLAB:UndefinedFunction`. Cycle 05 added the uncaught error's trace; the data is in the `MError`, but `e.stack` needs a struct array | `e.stack` in 07; the rest later |
| `exist` gives `5` for every builtin, where MATLAB gives `2` for the builtins it ships as `.m` files (`linspace`, for instance): every SplatCrab builtin is built in. What `exist` gives for a function local to the running script is not settled by the MathWorks page; SplatCrab gives `0`, and no case asserts it | by design; the local-function value later (verify first) |
| The trace names a function alone, `  in g3 (line 8)`, where MATLAB writes `Error in script>g3 (line 8)`; the spec fixes SplatCrab's form | by design |
| Functions are more permissive than MATLAB's in three ways, none of which changes what a file MATLAB accepts means: a function in a file on the path can call a local function of the script being run (invariant 4 reads the script's local functions after the running file's, where MATLAB keeps local functions private to their file); a file may mix functions that end with `end` and functions that do not; and a script's local functions may go without `end`. Calling a script with arguments or for a value is `Too many input arguments.` or `Too many output arguments.`, where MATLAB names the script | later |
| Command syntax judges "is a variable" when the source is lexed, from the workspace and the names assigned earlier in the source, so `x = 1; clear x; x -1` stays the expression; MATLAB judges a file the same way, the command line from the live workspace | by design; see cycle 04's Design notes |
| `warning('off')`, `warning('on')` and `lastwarn` do not exist: `warning('off')` prints `Warning: off`. `hold on` and `format long` are the unrecognized-name error | `hold` 12, `format` 13, warning state later |

A row that read "Char arrays display with quotes; MATLAB shows them bare" was
removed, because it misstated MATLAB. Since R2018a, `s = 'abc'` displays as
`s =`, a blank line and `    'abc'`, with the quotes, which is what SplatCrab
prints; `disp('abc')` is bare in both. A multi-row char shows a
`2×3 char array` header with each row quoted, which cycle 02 added and which
kept the quotes, as it was required to.

Cycle 02 removed four display rows it fixed: the `0.0000` zero, the untyped
empties, the narrow integer columns and the missing scale factor. The display
rules it chose where MATLAB's are not recorded, and the deviations it
accepted, are in the Design notes of `docs/modules/02-classes-and-display.md`;
several of its display values have not been checked against a real MATLAB
run, and that spec says which.

## Known bugs

Found while writing the cycle-0 unit tests, during the adversarial pass over
the baseline, while migrating the builtins in cycle 01, while closing cycle
01b, in the Phase 0 QA pass, and at the close of cycle 01c. The QA pass tested every FEATURES row, every
builtin and an adversarial set against MATLAB's documentation and GNU Octave
8.4. Its rows carry a "QA Dn" tag. Each row is recorded rather than silently
patched, and each is scheduled to a cycle. The table is grouped by that cycle.
Cycle 01 fixed the sixteen rows scheduled to it, including the two
process-killing panics. Cycle 01b fixed the four scheduled to it: the two
line-continuation defects, the missing `.\`, and the uncapped range. Cycle
01c fixed the eighteen scheduled to it, the builtin argument forms and the
QA pass's argument defects in the same builtins; it narrowed a nineteenth,
"Constructors take two sizes only", which stays. Cycle 01d fixed eleven of the
fifteen scheduled to it, including the three that aborted the process; it
removed a twelfth, `mod` and `rem` with an infinite divisor, without a change,
because the behaviour turned out to be right (see that cycle's Out of scope);
it narrowed the colon row to the infinite step alone and moved QA D16, the
`printf` spelling row, to cycle 11; and it left `for` over a matrix with no
rows where it found it. Cycle 01e fixed thirteen of the sixteen scheduled to
it, including the last one that aborted the process; it settled the
error-text row (QA D33) as a recorded policy rather than as a defect, moving
its two deliberate remainders to Known deviations above; it narrowed the
byte-order-mark row to UTF-16 alone; it moved the char-assignment row to
cycle 02, as its own Out of scope directs; and it left `1:NaN` unverified.
Fixing the non-finite row format discharged the display half of the row that
carried it and left a test-coverage half behind, so that half was a new row
scheduled to 02: three cycle-01 cases spelled with `fprintf` what their
bullets spell with `disp`. Cycle 02 fixed the six rows scheduled to it: char
indexed assignment, char rearrangement (QA D17), the scalar fixed-point range
(QA D20), the logical `disp` width (QA D38), the non-BMP character count
(QA D37, moved in from cycle 11) and the three cycle-01 cases, whose `disp`
lines came back and whose `% NOTE:` blocks went, as that row instructed. It
also turned QA D6 from a silent wrong answer into a clean error and narrowed
the `det` deviation above to its value half. Cycle 03 fixed the four rows
scheduled to it: logical indexing itself (QA D6), trailing singleton
subscripts (QA D22), the growth message that named the `usize` clamp, and
bracketed assignment targets (QA D32); it also discharged the Known
deviations row for the `x(0)` message, whose ending now names logical values.
Cycle 04 fixed the three rows scheduled to it: block comments that executed
(QA D7), `error`'s argument rules (QA D9) and command syntax (QA D31).
Fixed rows are removed from the table rather than marked done, but an
instruction a removed row carried is re-recorded, never dropped with it.

A row scheduled to a roadmap module that already lists it in its Scope stays
there. A row scheduled to a bug-fix cycle (01c, 01d, 01e) is written into that
cycle's spec when the spec is drafted. When a cycle fixes a row that a later
spec also lists, it removes the row from that spec in the same commit.

| Bug | Symptom | Fixed in |
|---|---|---|
| `1:NaN` is an empty, verify first | `1:NaN` is 1x0; Octave 8.4 gives the 1x1 `NaN` and MATLAB is unverified, so cycle 01d deliberately left it as it found it while refusing the infinite end points beside it. No golden case asserts either way | later (verify first) |
| `for` over a matrix with no rows iterates (QA D35), verify first | `for q = zeros(0, 3), disp(size(q)), end` iterates three times with `q` 0x1; Octave 8.4 iterates zero times. The MATLAB `for` page's "numel(valArray(1,:))" is ambiguous for a 0-row array. Do not encode either behaviour without a source that settles it. Cycle 01d left it as it found it | later (verify first) |
| A non-UTF-8 file is unread (was part of QA D29) | A UTF-16LE file is `Error: Line 2: unexpected character` on a NUL, the high byte of its first ASCII character (`err_utf16_file` pins that text and its exit code 1); MATLAB and Octave read it. Cycle 01e skipped the leading UTF-8 byte-order mark and swapped the strict read for a lossy one, which fixed the Windows-1252 half (a `% caf<E9>` comment now runs) and brought the failure inside the `Error:` format; a UTF-16 file still decodes to replacement characters rather than to its text, because that needs encoding detection and not a lossy decode | later |
| A colon operand that is not a scalar is an error | `[1 3]:4` is `range start must be a scalar.`, and so therefore is `1:2:3:4`, which cycle 01e taught the parser to read as `(1:2:3):4`. MATLAB is understood to take the first element of a non-scalar colon operand, which would make it `1:4`; that was not verified against a real MATLAB run, so 01e fixed the parse and left the evaluation as it found it. Verify before changing it | later (verify first) |
| `inv` and `A^-1` of a singular matrix are errors (QA D26) | `inv([1 2; 2 4])` exits 1 with "Matrix is singular to working precision."; MATLAB prints that text as a warning and returns `Inf Inf; Inf Inf`, and `inv(0)` is `Inf` (Octave the same). It needs `warning`, which cycle 04 added, and goes with the backslash deviation | 08 |
| `printf` conversions and flags differ from MATLAB (QA D16) | (a) `%E` and `%G` print a lower-case `e`. (b) `%s` of a non-integer uses `%g`: `sprintf('%s', pi)` is `3.14159` where the MATLAB `sprintf` page's own example gives `3.141593e+00`. (c) The `#` flag is ignored: `sprintf('%#.0f', 3)` is `3`, MATLAB `3.`. (d) The `0` flag pads a non-finite value: `sprintf('%05d', -Inf)` is `-0Inf`, MATLAB and C ` -Inf`. (e) The escapes `\xN`, `\N` (octal), `\a`, `\b`, `\f` and `\v` are not processed. (f) `%x`, `%X`, `%o` and a `*` width or precision are errors; MATLAB gives `ff` for `sprintf('%x', 255)` and `    3` for `sprintf('%*d', 5, 3)`. (g) An invalid conversion or a trailing `%` is an error; MATLAB "prints all text up to the invalid operator ... and discards the rest", so `sprintf('abc%q', 1)` is `abc`. Cycle 01d's Out of scope moved this row to 11: it is cosmetic, and cycle 11 rewrites `printf` for file output anyway. 01d kept the panics and the hang, which are not cosmetic | 11 |
| `num2str` of a matrix gives one row in column-major order (QA D13) | `num2str([1 2; 3 4])` is the 1x10 `'1  3  2  4'`; MATLAB gives the 2x4 char `'1  2'` / `'3  4'`. `num2str([1 -2 300])` spaces its columns differently too. It needed a multi-row char, which cycle 02 provides | 11 |
| `fprintf` rejects a file id and cannot return a byte count (QA D25) | `fprintf(1, 'hi\n')` is "The first argument must be a format string."; MATLAB writes `hi`, and `fprintf(2, ...)` writes to stderr. `n = fprintf('hi\n')` prints `hi` then "Too many output arguments."; MATLAB sets `n = 3`. Cycle 11 owns `fprintf(fid, ...)`; a file id of 2 writes to `Interp.err`, the second sink cycle 04 added for `warning` | 11 |
| `clc` writes raw terminal escapes to stdout | `clc` emits `ESC[2J ESC[H` through the normal output sink, so a script that calls it and is piped or redirected has those bytes in its captured output. MATLAB's `clc` affects the command window, not the program's output stream. Found while writing the handbook | 13 |
| `exit` and `quit` work only as bare REPL lines (QA D28) | A script ending in `exit` fails with "Unrecognized function or variable 'exit'." In the REPL, `exit;`, `quit;`, `exit(3)` and an `exit` inside a block are not recognised, and the final exit code is always 0. MATLAB's `exit` ends the session, and `exit(3)` exits with code 3. Cycle 13 claims `exit` | 13 |
| Constructors take two sizes only | `zeros(2, 3, 4)` is "N-D arrays are not supported."; MATLAB builds a 2-by-3-by-4 array. The same holds for `ones`, `rand`, `NaN`, `Inf`, `true`, `false`, `reshape` and `repmat`, with separate sizes or a size vector. Since cycle 01c a trailing size of `1` is dropped, as MATLAB drops it, so `zeros(2, 3, 1)` is 2x3, and any other third or later size, `0` included, is that clean error. Before 01c, `zeros`, `ones` and `rand` with three sizes were "Too many input arguments.", and before cycle 01 they built the 2-D array and dropped the third size. The row stays, because building N-D arrays needs a design that no roadmap module claims yet. `eye` is unaffected: MATLAB rejects `eye(r, c, p)` too | later, needs N-D arrays |
| Hex and binary literals are unsupported (QA D30) | `x = 0x1F` is `unexpected 'x1F'`; MATLAB R2019b+ and Octave give `31` | later, low impact |
| An unexpected character is echoed raw into the message | `unexpected character '<c>'` writes the character itself, so a control character reaches stderr as a raw byte: running `01e-display-and-parser/err_utf16_file.m` writes a literal NUL between the quotes. A control character should be named, for instance as `U+0000`. Found while rebuilding the test inventory after cycle U0 | later, low impact |
| Most builtins refuse an `MException` | `isa(e, 'MException')`, the usual MATLAB check, is an error, and so are `size(e)`, `isempty(e)` and `ischar(e)`; `who` and the protocol's `workspace` already show it as `1x1 MException`. Cycle 04's Scope named only `class`, `rethrow` and the two fields. Found by cycle 04's review | 07, with the other non-matrix values |
| Command syntax does not see an implicit `ans` in a script, verify first | Whether a name is a variable is judged when the source is lexed, from the names the script has assigned so far, and `3;` assigns `ans` only at run time. So `3;` then `ans -1` is the command `ans('-1')`, an index error, where the REPL and `--protocol`, which see the live workspace, read an expression. What MATLAB does is not settled by its documentation. Found by cycle 04's review | later (verify first) |

**The process-killing family.** The two panics that used to head this list,
`num2str(Inf)` and `zeros(1e10)`, were the first thing cycle 01 fixed, before a
single builtin moved. Cycle 01b closed the third, the uncapped range, by
routing the `:` operator through `args::check_size` like every builtin shape
(since cycle 01c, both go through `args::check_shape`, and since cycle 03
indexed growth does too).
The QA pass found that the family was larger than the two `printf` rows that
were left. Cycle 01d closed three of the four it added. `printf` bounds its
width and its precision, so no conversion panics and none builds a pad it
cannot afford. Every result shape computed from operand shapes now goes
through `args::check_shape` before anything is allocated: broadcasting in
`Matrix::try_zip`, `matmul`, every read and write through `resolve_read` and `resolve_write` (cycle 03), and `math::reduce`
(QA D1), which is the same guard that stops a `matmul` size wrapping (QA D2).

**Cycle 05 measured the evaluator again, and the 01e margin does not hold for
nested calls.** A chain of 10,000 builtin calls, `abs(abs(...1...))`, needed
198 MB of stack in a debug build at cycle 04's HEAD (46 MB in release), about
20 KB a level, far from the "about 28x" below, which was measured on a flat
sum. It still fits the 256 MB thread, and cycle 05, having shrunk `MError`,
brought it to 172 MB (44 MB in release). The debug figure is the one the
golden harness runs, so it is the margin to watch: see the Design notes of
`docs/modules/05-functions-and-scoping.md`.

**Cycle 01e closed the last member: deep nesting overflows the stack (QA D4),
so invariant 6 holds again.** The parser and the evaluator each count nesting
against `parser::MAX_DEPTH`, 10,000 levels, and refuse anything past it; see
the Design notes of `docs/modules/01e-display-and-parser.md` for how the
number was chosen and the margin it leaves. The fix had to be a limit rather
than a bigger stack, because the recursion is unbounded by nature: the 256 MB
stack moved the abort from about 5,000 levels to about 96,000 and could never
remove it.

"Holds again" is a claim about every input the project knows of, not a proof.
`src/main.rs` therefore still joins the interpreter thread with
`unwrap_or(101)`, not `unwrap_or(1)`: a panic must stay distinguishable from a
clean error by exit code, since that is the golden harness's main tripwire. A
case with an `.err` file expects exit 1, so a panic reported as 1 could pass a
test that was meant to prove the opposite. An allocator abort or a stack
overflow exits 134, which is distinguishable from both. A new one belongs in
this table, and a new golden case must assert the exit code and not only the
message text — a case that checks the message alone would pass on the very
abort it was written to catch.

One residual risk is recorded rather than fixed: if the 256 MB thread cannot
be spawned at all, `main` falls back to running on the main thread, whose 1 MB
would be exhausted well before 10,000 levels. Nothing observed has ever taken
that branch.

Three entries are marked "verify first": `for` over a matrix with no rows,
`1:NaN`, and the colon operand that is not a scalar, which cycle 01e added
when it taught the parser to read `1:2:3:4`. All three were found by reasoning
about MATLAB rather than by running it, and neither the MATLAB pages nor
Octave settles them. Confirm the real behaviour before writing a test that
asserts either way: an expected-output file that encodes a guess is worse than
no test.

Two former "verify first" rows are gone. The loop variable after a
zero-iteration `for` was settled as a real bug and cycle 01d fixed it. `mod`
and `rem` with an infinite divisor went the other way: Octave 8.4 and MATLAB's
own documented formulas `a - m.*floor(a./m)` and `a - b.*fix(a./b)` all give
the `NaN` this interpreter already gives, and nothing supports the `5` the row
once suspected, so cycle 01d removed the row without a change. That is still
not a real MATLAB run, so **no golden case asserts either value**: the
evidence is strong enough to stop calling it a bug, not strong enough to pin.
