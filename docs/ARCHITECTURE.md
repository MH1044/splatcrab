# Architecture

```
 .m source ──► lexer.rs ──► parser.rs ──► interp.rs ──► value.rs
              tokens       Stmt / Expr   tree-walking   column-major
              + lines      + lines       evaluator      f64 matrices
                                              │         + a class tag
                                              │
                                         builtins/
                                         the library, behind a registry
                                              │
                                         plot/        figures, and their
                                         figure.rs    layout into a scene,
                                         svg.rs       written as SVG or
                                         png.rs       rasterized into a PNG

                            error.rs: MError, and every message text

 JSON line ──► protocol.rs ──► json.rs      parse the request, write the reply
                   │
                   ├──► interp.rs           eval, output captured
                   ├──► syntax.rs           complete: is the entry finished?
                   └──► env.rs              completions: variables + path files + builtins

 browser ──► server.rs ──► http.rs ──► protocol::respond
 127.0.0.1   accept, read   limits, Host, Origin, token, routes
 only        one request,   │
             write, close   └──► src/ui/    index.html, app.js, app.css,
                                            embedded with include_str!
 stdin ────► http::serve_stdio (--http-stdio): the same http::handle, no socket
```

`src/lib.rs` exposes the fifteen modules: the six of the language (`lexer`,
`parser`, `interp`, `value`, `builtins`, `error`), the four of the
evaluation protocol that cycle U0 added (`json`, `syntax`, `env`,
`protocol`), the two of the UI server that cycle U1 added (`http`,
`server`), cycle 12's `plot`, and cycle 13's `editor` and `history`. `src/ui/` holds the page's three files, which `http.rs` embeds.
`src/main.rs` is the CLI and REPL and, with `src/term.rs`, the raw-mode
terminal module only the binary compiles, is the only code allowed to use
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

Since cycle 10 a number followed straight away by `i` or `j`, and not by
more of a name, is an imaginary literal, `Token::Imag`: `1i`, `2.5j`, `1e3i`,
while `2ix` is still `2` and the name `ix`. The bare names `i` and `j` stay
identifiers, which the evaluator resolves by invariant 4, so a variable of
the name shadows the builtin unit. `.'` became a token of its own,
`DotTranspose`, the plain transpose, and `'` after a value the conjugate one.

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
the whitespace rule treats as a bracket, so `case {2 3}` is two values. Since
cycle 07 so does every brace that does not follow the end of a value, which
is a cell literal: `{1 -2}` is two elements and a newline in it starts a
row, while in the brace index `c{1 -2}` the whitespace separates nothing.

Since cycle 03 the lexer also has `{`, `}`, a lone `.` and `@`. The field dot
is whatever dot is left once a number's decimal point, the five dotted
operators (`.*`, `./`, `.\`, `.^`, `.'`) and a `...` continuation have been
recognised, so none of those changed. A `}` ends a value, so `c{1}'` is a
transpose. Braces go on the delimiter stack like brackets, so that inside
`[c{1 -2} 3]` the whitespace rule applies to the bracket and not within the
braces; inside brackets a `{` or an `@` after a space starts a new element.

Since cycle 06 the lexer knows an anonymous function's shape, for the same
reason: whitespace inside its body separates nothing. The `(` straight after
an `@` goes on the delimiter stack as `P`, and when its `)` closes it with a
bracket or a brace directly around, an `A` goes on for the body, which the
whitespace rule does not treat as a bracket. The `,`, `;`, newline or closer
that ends the element pops it, so `{@(x) x + 1, 2}` is two elements and
`[@(x) x+1]` one. A quote straight after that `)` opens a string rather than
a transpose, so `@() 'hi'` is a function returning text.

**`parser.rs`** is recursive descent with MATLAB's precedence, loosest first:
`||`, `&&`, `|`, `&`, comparison, `:`, `+ -`, `* / \ .* ./ .\`, unary `- ~`,
`^ .^`, transpose (`Expr::Transpose` for `'`, `Expr::DotTranspose` for `.'`,
since cycle 10, beside the literal `Expr::Imag`). `end` and a bare `:` are only accepted inside an index
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
rewinds so that the bracket parses as the matrix literal it always was. A `{`
where a value should start is a cell literal, `Expr::Cell(rows)` (cycle 07),
parsed like a bracket but taking handles as elements and wanting a
separator after each one, so `{@(x) x 1}` is refused.

Cycle 06 added the two handle forms, `Expr::FuncHandle(name)` for `@name` and
`Expr::AnonFn(Rc<AnonFn>)` for `@(params) body`. The body is one expression,
parsed with `in_index` cleared, since it is not an index argument even inside
one. `AnonFn::new` records the body's free names, every name it reads that is
not a parameter, once at parse time, because every evaluation of the `@(...)`
captures those of them that are variables. A handle as an element of a
bracket, `[@(x) x+1]` or `[1 @sin]`, is refused here. `parser::render` turns
an expression back into source text for `func2str`: no spaces around binary
operators, commas between bracket elements, and parentheses only where the
precedence needs them, since the tree keeps none; a rendered body parses back
to the same tree. `Parser::parse_handle` reads a `str2func` text, one handle
form and nothing after it.

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
code unit. Since cycle 10 a double can be complex: `im: Option<Vec<f64>>`
holds the imaginary parts, column-major like `data`, and is `None` for real
storage; see "Add a value type". `value.rs` also owns the display: `format` for the numeric body,
`disp_text` for `disp`, and `display_body` for the class headers of a named
display. Since cycle 04 `Value` has a second variant, `Exception`, the
`MException` a `catch` binds; `Value::mat` and `Value::into_mat` return an
`R`, refusing it with `This operation is not supported for a value of class
'MException'.` Since cycle 06 there is a third, `Func(Rc<Func>)`, a function
handle, refused the same way with its class `function_handle`; see "Add a
value type". Cycle 07 added the containers, `Cell(Rc<CellArray>)` and
`Struct(Rc<StructArray>)`, their displays, and the drop worklist that frees
every nesting value without recursion; see "Containers" below.

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

Since cycle 06 it makes and calls function handles. `named_handle` binds
`@name` to the local function the name resolves to where the handle is made,
or to nothing, in which case the call resolves the name against the path and
the builtins (`call_global`, the half of `call_function` after the local
functions). `anon_handle` snapshots the free names that are variables of the
running frame, and the frame's `unit`. `call_handle` calls either kind, and
`call_anon` runs an anonymous function in a frame of its own holding the
captures and the parameters, counted against `MAX_RECURSION`, the body run
through `eval_request`. `eval_request` is what a statement, a multiple
assignment and an anonymous body share: a call, by name or of a handle
variable with one `(...)`, is asked for exactly the outputs wanted, an
access chain gives its cs-list (cycle 07), and any other expression is its
one value, which is how `nargout` passes through a body that is a single
call.
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
argument helpers, and `core.rs`, `math.rs`, `linalg.rs` and, since cycle 07,
`cells.rs` (cells, structs, `cellfun` and the map `arrayfun` shares) and,
since cycle 09, `numerics.rs`, `sets.rs` and `solvers.rs` the builtins
themselves. Every one has the same shape,
`fn(&mut Interp, &[Value], usize) -> R<Vec<Value>>`, where the `usize` is
`nargout` and an empty `Vec` means the builtin produced no value.

Since cycle 08 `factor.rs` holds the numerics of linear algebra, with no
`Interp` in them: `lu` (the one LU with partial pivoting that `det`, `inv`,
`Matrix::solve`, `A^-n` and the `lu` builtin share, returning an `Lu` whose
`singular` flag is the `Matrix::singular_tol` test with `<=`), Householder
`qr` and the column-pivoted least-squares `lstsq`, `chol`, the cyclic Jacobi
`eig_sym`, `eig_general` (EISPACK's `orthes` and `hqr2`, as JAMA transcribes
them; since cycle 10 a complex pair is two complex values with complex
vectors, from JAMA's complex back-substitution) and the one-sided Jacobi
`svd`, which returns
no vectors, thin ones or full ones as `Vectors` asks. Every iteration there
takes its cap as a parameter (`JACOBI_SWEEPS`, `SVD_SWEEPS`,
`qr_iterations(n)`) so that a unit test can reach the `no_convergence` error.
A computation that warns returns its warning text beside its result, as
`Matrix::solve` and `Matrix::inv` return `(Matrix, Option<String>)`, and the
caller writes it with `Interp::warn`, through `Interp.err`; `factor.rs` and
`value.rs` never write anything themselves.

Cycle 09 added three files. `numerics.rs` holds the builtins on data:
the polynomials (`polyfit` over `factor::lstsq`, `roots` over
`factor::eig_general` of the companion matrix), interpolation, the
trapezoidal rule, differences, `filter`, the statistics, the number theory
and the grids. Its `map_slices` is the one way a function there works
along a dimension: it hands each column (or each row, through a
transpose) to a closure, and judges the result's shape with `check_shape`
first; `first_dim` is MATLAB's default dimension. `sets.rs` holds the five
set functions over a `Set`, either an array compared as numbers or a cell of
character vectors compared as code-unit texts. `solvers.rs` holds `fzero`,
`fminsearch`, `integral` and `ode45`, each a thin builtin around a pure
method (`fzero_solve`, `nelder_mead`, `quad`, `dopri`) that takes the user's
function as a Rust closure and its caps as parameters, so a unit test can
drive the numerics and reach every cap with no interpreter. The builtin's
closure calls the user's function through `Interp::call_nested`.

Cycle 10 added `complex.rs`: `C`, the complex scalar every complex kernel
computes with (the arithmetic, `sqrt`, `ln`, `log2`, `log10`, `exp`, `sin`,
`cos`, `asin`, `acos` and `pow`, each real wherever its real counterpart
is, with the branch cuts its module comment records), the builtins of
complex numbers (`real`, `imag`, `conj`, `angle`, `isreal`, `complex`, and
`i` and `j`), and `fft` and `ifft`: a radix-2 Cooley-Tukey transform for a
power-of-two length and Bluestein's algorithm, a chirp convolution through
power-of-two transforms, for every other, so every length is O(n log n).
`builtins::TAKES_COMPLEX` names the builtins that take a complex argument,
and `complex_gate`, which `Interp::call_builtin` runs before every builtin,
refuses one to any other; see "Add a value type".

Cycle 11 added five files. `printf.rs` is the formatter the `printf`
family shares, moved out of `core.rs` and finished (QA D16): `%x`, `%X`,
`%o`, `*` fields, the `#` flag, the remaining escapes and MATLAB's rule
that an invalid conversion ends the output. Every width and precision,
written or given by `*`, is still judged against `printf::MAX_FIELD`
before it reaches Rust's formatter, whose panics it prevents.
`strings.rs` holds the string functions, `num2str` among them (one row
per matrix row since QA D13), and `regexp` and `regexprep` over
`regex.rs`, a Pike VM: a pattern is parsed into a tree, compiled into a
program of at most `regex::MAX_PROGRAM` instructions, and run by
simulating every thread at once, one thread per program counter per
position, so a search is `O(n * m)` whatever the pattern. Finding every
match is one pass too: the next search starts inside the same pass as
soon as the current one has a match, and a thread of a later search is
dropped where an earlier one holds its program counter. The parser and
the compiler recurse once per group, bounded by `regex::MAX_NESTING`;
a backreference, which no engine can match in linear time, is refused
there. `io.rs` holds `input`, the file-identifier table
(`io::FileTable`, on `Interp.open_files`, identifiers from 3 up, the
lowest free first), the functions that read and write through it, the
whole-file functions, `delete`, and `save` and `load`, which read and
write MAT-files through `mat.rs`: pure functions over bytes, the reader
treating the file as untrusted (see invariant 6). Every path any of them
names goes through `Interp::resolve_path`, against `Interp::cwd`, and a
function that writes or deletes a file calls `Interp::files_changed`, so a
`.m` file a script writes is found by the next call. `Interp.input` is
where `input` reads: standard input, through the one buffer the REPL
reads, or `InputSource::Refused`, which `protocol::eval` puts in place for
the length of every call, so under `--protocol`, `--ui` and
`--http-stdio` an `input` is a clean error rather than a read of the next
request.

**`plot/`** is cycle 12's plotting, in four files. `mod.rs` holds the
twenty builtins and their argument rules, registered from
`builtins::registry` like any other; they only change state. `figure.rs`
holds that state, `Figures`, which `Interp.figures` is: every open figure by
number (a `BTreeMap`), the order figures were last made current in, whose
last is the current figure, and the numbers changed since the REPL last
asked. A `Figure` holds its `Axes`, one per subplot, each placed by a
rectangle in fractions of the figure, and the current one; an `Axes` holds
its `Series` (a line, a scatter or bars, each a copy of the user's data),
its hold state, labels, legend, grid and limits. `figure.rs` also parses
line specs, holds the tick rule, and lays a figure out, `scene`, into one
flat list of drawing `Item`s in pixels of the 560x420 figure: groups,
optionally clipped, rectangles, lines, polylines, markers and texts.
`svg.rs` writes a scene as SVG text and `png.rs` rasterizes it and encodes
the pixels, so the two formats read one layout and cannot disagree. The
SVG vocabulary, the tick rule, the font and the rasterizer's coverage are
in the Design notes of `docs/modules/12-plotting.md`.

Nothing is drawn until a figure is saved, printed or asked for:
`Interp::figure_svg(n)` renders figure `n` from what it holds and
`Interp::figure_numbers()` lists the open ones, so a front end can show a
figure inline without a file (cycle U4 adds the protocol operation). A
plotting call is judged whole before it copies or changes anything: its
data's pairing, then the figure's point budget (`figure::MAX_POINTS`),
counted from the arguments in place, each thing drawn weighted by the SVG
it writes, then the copy and the change.

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
`completions(prefix, &vars, &registry, &path)` lists every variable, every
function file (`name.m`, listed as `name`) in the `path` folders and every
builtin starting with `prefix`, sorted by byte order and deduplicated.
`Interp::builtins` hands out the registry read-only for it and
`Interp::path_dirs` the folders, the current one first. The protocol's
`completions` and the terminal's Tab both call it (cycle 13).

**`editor.rs`** (cycle 13) is the terminal's line editor as a pure state
machine: `Decoder` turns the characters a terminal sends, ANSI escape
sequences included, into `Key`s, and `Editor::key` turns a key into a new
buffer, cursor and history place, or an `Action` (submit, cleared by
Ctrl-C, end of input, a list of completions), asking a completion function
it is handed for Tab. It reads and writes nothing, so every key is unit
tested. **`history.rs`** is the history file: UTF-8, one entry per line,
oldest first, with `\`, `\n` and `\r` escaped so a multi-line entry stays
one line; appended to one entry at a time and compacted on load past twice
its 1000 entries, at `SPLATCRAB_HISTORY` or `~/.splatcrab_history`. It is
the format the interface is to share. **`src/term.rs`**, a module of the
binary alone, is the raw-mode shell around the editor: `LineReader`
enters raw mode for one line at a time through raw declarations
(`GetConsoleMode`, `SetConsoleMode` and `ReadConsoleW` with virtual-terminal
input and output on Windows; `tcgetattr`, `tcsetattr` and `cfmakeraw`
elsewhere), feeds the decoder, draws the line with `print!`, and restores
the terminal in a guard's `Drop`, so every exit path restores it.

**`builtins/environ.rs`** (cycle 13) holds `cd`, `pwd`, `ls`, `dir`,
`help`, `which`, `format`, `eval`, `evalc`, `run`, `datestr`, `now`,
`clock`, `pause`, `getenv`, `system`, `version`, `exit` and `quit`; `who`,
`whos` and `clc` stay in `core.rs`. `now` and `clock` read local time
through `GetLocalTime` or `localtime_r`, declared raw like the terminal's
calls. `eval` and `run` are `Interp::eval_code` and `Interp::run_path`,
each counting one level of the nesting budget; `evalc` swaps both sinks
for a buffer, as a protocol `eval` does. `exit` is `MError::exit(n)`, an
error value that is not an error: it leaves every frame and builtin as an
error does, `try` and `eval`'s fallback let it through, and `main.rs`
exits with its code; where `Interp::input` is `Refused`, under
`--protocol`, `--ui` and `--http-stdio`, it is a clean error instead.

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

**`main.rs`** is the CLI. Since cycle 13 it answers `--help` and
`--version` (the version is `env!("CARGO_PKG_VERSION")`, as the banner's
is), sets `Interp::stdout_tty` and `Interp::stdin_tty` from
`IsTerminal`, which `clc` and a bare `pause` read, exits with the code of
an `exit(n)` that reaches it in a script or at the prompt, and reads the
REPL's lines through `term::LineReader` when standard input and standard
output are both terminals, and as plain lines otherwise, exactly as before.
Since cycle 12 its REPL shows figures: when
standard input is a terminal (`std::io::IsTerminal`), after each entry it
asks `Interp::take_changed_figures` for the figures the entry changed,
writes each to `splatcrab-<pid>-figure-<n>.svg` in the temporary folder
and opens it with `server::open_browser`'s launcher, ignoring any failure.
That is the only place a viewer opens: no builtin opens one, and a script,
a golden case, CI (whose standard input is never a terminal), `--protocol`,
`--ui` and `--http-stdio` never reach that code. On Windows it first switches the console's output
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
   the recipe below. An iteration that has no fixed trip count has a cap
   too, and a clean error past it: since cycle 08 the Jacobi sweeps, the
   one-sided Jacobi SVD and the shifted QR iteration of `factor.rs`, whose
   callers refuse a `NaN` or `Inf` before the first sweep, so no input can
   make one spin; since cycle 09 the solvers of `solvers.rs`: `fzero`'s
   search for a sign change and its Brent iterations, `fminsearch`'s
   iterations and evaluations, `integral`'s subintervals and `ode45`'s
   steps and minimum step, each past its cap a clean error rather than a
   `NaN` handed back.
   Since cycle 11 input that is not the program's own is held to the same
   rule: the regular-expression engine runs in time linear in its subject
   whatever the pattern, and bounds the pattern's nesting and compiled
   size, each class built once into ranges that count toward that size;
   the MAT reader checks every length against the bytes there and
   every array's dimensions through `check_shape` before it allocates,
   never reserves more elements than the bytes left could hold, bounds a
   struct array with no fields, which no bytes pay for, by
   `mat::MAX_FIELDLESS`, and bounds cells and structs nested in a file by
   `mat::MAX_DEPTH`;
   since cycle 12 a figure holds at most `plot::figure::MAX_POINTS` points,
   a marker, a circle, a bar, a line's run and a series each weighted by
   the SVG it writes, judged from the arguments before a plotting call
   copies or changes anything, so no figure writes much more than 256 MiB
   of SVG; and every segment the rasterizer walks is clipped to its clip
   box first, a wide one drawn a run across it a pixel along, so drawing
   costs time linear in the points whatever their coordinates;
   `fileread`, `fgetl`, `fgets` and `input` stop reading text at
   `io::MAX_TEXT_BYTES`, past which no char row could hold it, and the rows
   of numbers read from a text file are judged by `check_shape` as each is
   added; and `printf` bounds a `*` width or precision as it bounds a
   written one.

## Recipes

### Add a builtin

1. Write the function in the right file under `src/builtins/`: `core.rs` for
   constants, constructors, shape queries, output, the workspace and timing;
   `math.rs` for element-wise and reducing numerics; `linalg.rs` for linear
   algebra, rearrangement, search and sort, with the numerics of a
   factorisation in `factor.rs` and only the argument handling in
   `linalg.rs`; `numerics.rs` for polynomials, samples, statistics and
   number theory, `sets.rs` for the set functions, `solvers.rs` for a
   builtin that iterates on a user's function, and since cycle 11
   `strings.rs` for text and `io.rs` for input and files. A plotting
   builtin goes in `src/plot/mod.rs`, whose `register` the registry calls. The signature is
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
6. If it takes complex values (cycle 10), handle `Matrix::im` and add its
   name to `builtins::TAKES_COMPLEX`. Otherwise do nothing: the registry
   refuses a complex argument to it before it runs, so it can read `data`
   as the real array it is.

Use the helpers in `src/builtins/args.rs` for argument access; they produce
the MATLAB-style messages. `need` and `at_most` bound the argument count,
`mat`, `scalar` and `string` fetch one, and `option` returns a char option
such as `'descend'` for the caller to match; both read a char argument's
UTF-16 code units back into a Rust `String`. `dim` reads a dimension argument
(a positive integer, never a char), and `dim_or_all` also accepts `'all'`,
for the reductions that take it.

**A builtin that calls a function goes through `Interp::call_nested`**, with
a `Callee::Name` or a `Callee::Handle`, never through `call_function` or
`call_handle` directly: `call_nested` counts the call as one level of the
nesting budget every frame shares, which is what bounds a function that calls
a builtin that calls the function (cycle 05's review found `feval` re-entering
uncounted and overflowing the stack). `feval` and `arrayfun` do so since
cycle 06, `cellfun` since cycle 07, and the solvers `fzero`, `fminsearch`,
`integral` and `ode45` since cycle 09, once per evaluation, so a solver
calling a solver, or a function calling a solver on itself, stays bounded.

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

A cell or struct element is far larger than a double, so since pass 13b a
cell or struct array is judged in bytes too, by `check_bytes` (through
`check_cell` and `check_struct`): the same 2 GiB budget, `MAX_BYTES`, that
`MAX_ELEMS` doubles take, with a cell element counted as
`size_of::<Value>()` and a struct element as that times its field count plus
one. It comes after `check_shape` and refuses in `check_shape`'s own words.
Every place that makes or grows one calls it before allocating: `cell`,
`struct`, `num2cell`, `cellfun` and `arrayfun` with `UniformOutput` false, a
cell literal, `c(idx)` and `s(idx)` reads, every indexed growth (a new field
of a struct array included), concatenation, and `load` (where a cell the
file's bytes cannot hold still fails as the truncation it is). `repmat`
takes no cell, and `deal` makes none of its own.

The mirror rule is that an empty result costs nothing. Since pass 13b every
element-wise kernel, real and complex (`try_zip`, `zip_c`, `transpose`),
the running scans, `fft`, `sort`, `kron`, `repmat`, `fliplr`, `flipud`,
`triu`, `tril`, `numerics::map_slices`, vertical concatenation, and the
index resolvers (`resolve_read`, `resolve_write`, `resolve_delete`,
`Sel::covers`) loop over the elements they produce or read, never over a
dimension of an empty operand, so `zeros(0, 1e12) + 1` returns at once and
`x(:, :)` of it lists no positions. `for` over an array with no rows still
iterates its columns, which is the open question of its own Known bugs row.

A shape the user never spells out goes through `check_shape` too. Since cycle
01d, any operation whose result shape is computed from its operands' shapes
calls it before allocating: `Matrix::try_zip` (and so `zip`), `Matrix::matmul`,
the two-subscript branch of `resolve_read` and `math::reduce`, and since cycle
08 `kron` and every shape `factor.rs` computes (through `factor::zeros` and
`factor::eye`: the `n`-by-`k` answer of a system with no rows, the `m`-by-`m`
`Q` of `qr` and `U` of `svd`, `lu`'s `P`), which is why `svd` of a long
column asks for its `U` only when the full decomposition was requested.
Cycle 09's shapes go the same way: `numerics::map_slices`, the
Vandermonde matrix of `polyfit`, the combinations of `nchoosek(v, k)`
(counted exactly first), the sieve of `primes`, `meshgrid`, `histc`,
`interp1` of a matrix and `ode45`'s output, judged before each step adds
its points. Cycle 11's too: every array a MAT-file declares, the `[m n]` of `fread`
(judged by the elements read, never by the size asked), the matrix
`readmatrix`, `csvread` and `load -ascii` build from rows of text, whose
longest row times their count can dwarf the file, and `blanks`. Cycle 12's
too: a PNG's pixel size, from the figure's size and `print`'s `-r`, before
the image is allocated, and `histogram`'s bin count. The operands can
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

Cycle 10 gave `Matrix` a second storage field rather than a class:
`im: Option<Vec<f64>>`, the imaginary parts, column-major like `data`,
`None` for real storage. A complex value is still class `double`, so
`class`, the class queries and the protocol's `workspace` need nothing new,
and a complex logical or char never exists: `to_class` refuses to make one.
The flag rule is MathWorks': every operation stores its result real when
every imaginary part is zero, through `Matrix::with_im` (and `from_c`,
`map_c`, `zip_c`), and `complex(a, b)`, `Matrix::complex_parts`, is the one
way to keep a zero imaginary part. Indexing, concatenation, deletion,
transposition and indexed assignment follow the same rule, so `z(2)` of
`[1+2i 3]` is a real `3`; only a copy of a whole value (an assignment
`y = x`, an argument, an element of a cell or a field) keeps a
`complex(1, 0)` complex. A double target assigned a complex value becomes
complex, the analogue of the class rule; a logical or a char target
refuses one.

A kernel that reads `data` alone would drop the imaginary part in silence,
the one failure this design must never allow. Two guards stop it: the
registry refuses a complex argument to every builtin not on
`builtins::TAKES_COMPLEX`, and the interpreter's own consumers of `data`
each handle `im` or refuse it (`truth` and `logical_scalar` for `if`,
`while`, `&&` and `||`; `&`, `|` and `~`; a complex index or `:` operand;
`to_class`). A solver refuses a complex value its function returns, and
`args::scalar` a complex dimension, size or option of a builtin that takes
complex data. Anything new that reads `data` must do one or the other.

A container is a different kind of value, not a class of array. Cells and
structs (cycle 07) become new `Value` variants beside `Mat`, as the
`MException` of cycle 04 already is: `Value::Exception` holds the caught
`MError` whole, so `rethrow` raises it unchanged, and it answers
`class_name`, `dims`, `display_body` and `disp_text` itself. Every array
operation reaches a matrix through `Value::mat` or `Value::into_mat`, which
return an `R` and refuse any other variant, so a new variant is refused
everywhere by default and each operation that should accept it says so; a
`match` on `Value` makes the compiler list the sites that must decide.

Cycle 06 added the third variant that way. `Value::Func(Rc<Func>)` is a
function handle: `Func::Named { name, local }`, with `local` the local
function and its file that `@name` resolved to where it was made, or
`Func::Anon { def, captured, unit }`, the parsed `AnonFn`, the snapshot of
the variables it captured and the file it was made in. It sits behind an
`Rc`, because reading a variable clones its value and a handle's captured
workspace should not be copied each time. It answers `class_name`
(`function_handle`), `dims` (1x1), `display_body` (`  function_handle with
value:`, a blank line, `    @(x)x+1`) and `disp_text` (`@(x)x+1`, `@sin`)
itself. The operations that accept it: a call through a variable or a chain,
`feval`, `arrayfun`, `func2str`, `class` and `isa`; `hcat` refuses it with
the concatenation message rather than the generic one.

Cycle 07 added the fourth and fifth, the containers `Value::Cell(Rc<CellArray>)`
and `Value::Struct(Rc<StructArray>)`. Each answers `class_name` (`cell`,
`struct`), `dims`, `display_body` and `disp_text` itself, and `Value::element`
gives element `k` of any value as a value of its own, which `arrayfun` and
`num2cell` hand out. Every value now answers the shape and class queries
(`size`, `numel`, `length`, `isempty`, `isscalar`, `isvector`, `isa`,
`class`, `islogical`, `ischar`, `isnumeric`): a handle and an `MException`
are 1x1. A binary operator on any value that is not a matrix is MATLAB
R2020a's `Operator '+' is not supported for operands of type 'cell'.`, from
`Interp::binary`, which evaluates both operands and names the first that is
not an array; every other operation still reaches a matrix through
`Value::mat` and keeps the generic refusal.

A value kind that holds other values must go on the drop worklist in
`value.rs`: `holds_values` names the kinds that nest, and `free` opens each
one it is the last owner of and moves what it holds onto the list, so no drop
recurses more than one level. `Drop` for `CellArray`, `StructArray` and `Func`
hands their contents to it.

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
and at the REPL alike. The protocol does not send it. Since cycle 07 an
entry also records the file of the function (`MError::leaving_file`, empty
for a function local to the code that was run), and `e.stack` reads the
entries as an Nx1 struct array of `file`, `name` and `line`.

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

**The current folder (cycle 13, in place).** `Interp::cwd` is the one
current folder: every path lookup, every file builtin, `ls`, `dir`, `run`
and `system` resolve against it, and `cd` changes it through
`Interp::set_cwd`, which normalises `.` and `..` by their components,
refuses a path that is not a folder, and bumps the lookup generation. The
process's working directory is never changed, so the golden harness's
per-case folder and the interface's file root hold whatever the code does.

**Display format (cycle 13, in place).** `format` is `Interp::format`; the
display reads it through `value::with_format`, which sets a thread-local
for the length of one display and puts the old value back, so a display
built outside the interpreter, as a unit test builds one, is always short.

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
Since cycle 06 an anonymous function's call is a frame too, with no
`func_name` (so `nargin` inside one is the outside-a-function error), the
file the handle was made in as its `unit`, and its own `end_stack`; it counts
against `MAX_RECURSION` like a user call. An error leaving it gains a trace
entry named by its `func2str` text, `  in @(n)g(n)`, with no line, since an
expression has none.

**Containers (cycle 07, in place).** `CellArray` and `StructArray` are
separate types, not a generic `Matrix`, and both reuse the index pipeline:
`eval_index_args` makes the `Sel`s, `resolve_read`, `resolve_write` (which
takes the right-hand side's shape, not a matrix) and `resolve_delete` plan
against the container's shape, and `pick`, `regrid` and `keep_positions`
carry the plan out on its column-major items, as `gather`, `scatter` and the
deletion do on a matrix's. A `CellArray` is `rows x cols` values; a
`StructArray` is `rows x cols` elements, each one value per field of
`fields`, in the order the fields were first made. A struct with 32 fields
or more also keeps a hash index from name to place, built on its first
lookup and kept current by `ensure_field`, so adding fields one at a time is
not quadratic; it is private, so every struct is made by
`StructArray::new` or `scalar`. Both sit behind an `Rc`
and are copied on write (`Rc::make_mut`), so reading a variable, `c = {c}`
and a capture are pointer copies, and `c{end+1} = k` in a loop mutates in
place.

A read is a cs-list: `eval_access` returns `Vec<Value>`, one value for most
chains and one per selected element for `c{...}` and for a field of a struct
array. `eval_multi` spreads it where MATLAB does, into a call's arguments and
the elements of `[...]` and `{...}`, `eval_request` hands it to
`[a, b] = c{:}`, and everywhere else `one_value` turns any length but one
into `Expected one output from a curly brace or dot indexing expression, but
there were N results.`. The leading fields of a variable are walked by
reference, so `s.data(k)` reads in place as `x(k)` does.

A write goes through `assign_to` for every chain: `resolve_links` evaluates
each subscript against the shape the path holds at that point (`shape_at`,
`0x0` where it holds nothing), so `end` means what a read would mean, and
turns dynamic fields into names; then the recursive `assign_chain`, bounded
by `MAX_DEPTH` links, stores the value. It creates the right empty
container where the path does not exist yet (a field of `[]` makes a struct,
a brace a cell, `p(2).name` a struct array) and grows with `[]` elements.
Nothing is created or grown until the assignment below it has succeeded,
and a variable made for the walk is removed on failure, so a failed
assignment changes nothing. `delete_at` and `nav_mut` delete at the end of a
longer chain, `s.list(2) = []`.

## Known deviations from MATLAB

Recorded as `% NOTE:` lines in the affected golden cases, and fixed in the
cycle named:

| Deviation | Fixed in |
|---|---|
| `det([1 2; 3 4])` prints `    -2`, where the spec records MATLAB's `   -2.0000`. Cycle 02 fixed the display half: a value a rounding error from an integer now prints with decimals. The value half remains: this interpreter's pivoted elimination lands exactly on `-2`, because the last product `3 * 0.66666666666666674` is a rounding tie that goes to the even `2`, so there is nothing for the display to show. MATLAB's `-2.0000` implies LAPACK returns `-2.0000000000000004`, an operation order not reproduced here. Cycle 08 replaced `det` with the shared LU and kept the old elimination order on purpose, since no source at hand settles LAPACK's; its spec forbids choosing an order for the digits it gives | later (verify first) |
| An error text says more than MATLAB's and keeps its own wording: the dimension mismatch names the operator and both shapes, where MATLAB says only `Arrays have incompatible sizes for this operation.` | by design; see the message-text policy in `docs/modules/01e-display-and-parser.md` |
| Numerics, cycle 09: a solver that fails (`fzero` with no sign change, `fminsearch` at its cap, a divergent `integral`, `ode45` below its smallest step) is a clean error, where MATLAB warns and returns a value or `NaN`; `polyfit` with too few points warns with `\`'s rank-deficient text; `ode45` with one output gives a struct of `solver`, `x` and `y` only; several message texts are SplatCrab's own. The Design notes of `docs/modules/09-numerics.md` have each. Cycle 10 replaced the complex refusals of `roots` with the values | by design |
| Linear algebra, cycle 08: a system singular only to working precision warns with MATLAB's exactly-singular text, where MATLAB is understood to say "close to singular or badly scaled" with an `RCOND`; a rank-deficient least-squares system warns with SplatCrab's own `Matrix is rank deficient to working precision (rank r).`; `det` is exactly `0` wherever `\` warns, where MATLAB's is the product of the pivots; `eig`, `svd`, `rank`, `pinv`, `null`, `orth` and `cond` refuse a `NaN` or `Inf`; `eig([])` is 0x1. The Design notes of `docs/modules/08-linear-algebra.md` have each | later (verify first) |
| Complex numbers, cycle 10: every builtin not on `builtins::TAKES_COMPLEX` refuses a complex argument (`sort`, `max`, `min`, `floor`, `mod`, `num2str`, `reshape`, `inv`, `det`, the solvers and every other), where MATLAB takes many of them; `if`, `while`, `&`, `\|`, `~`, `&&` and `\|\|` refuse a complex value; indexing, concatenation and assignment drop an all-zero imaginary part as arithmetic does, and a zero imaginary part of either sign is read as `+0`, on a branch cut and in the display; the complex display, its scale factor and the phase of complex eigenvectors are SplatCrab's; `eig` and `roots` of complex input are refused. The Design notes of `docs/modules/10-complex.md` have each | by design (verify first) |
| Chained indexing `x(2:3)(2)` is read successively, as Octave does; MATLAB refuses it. `x()` is "Only 1-D and 2-D indexing is supported." where MATLAB returns `x`. Both recorded in cycle 03's Design notes | later |
| Indexing into or growing a second page, `A(:, :, 2) = 5` or `A(:, :, [1 1])`, is "N-D arrays are not supported."; MATLAB builds the N-D array. Cycle 03 accepted it | later, with N-D arrays |
| An `MException` is minimal: `message`, `identifier`, `stack` and `class`, with no `cause` or `Correction`, and its display is SplatCrab's one line `  MException (id): msg` rather than MATLAB's property listing. An error the interpreter raises itself has an empty identifier, where MATLAB's carry one such as `MATLAB:UndefinedFunction`. `e.stack` (cycle 07) holds the function frames only, not the script's own, and `file` is empty for a function local to the script that was run | later |
| Cells and structs, cycle 07: a cs-list is not spread into index subscripts (`x(c{:})`) nor accepted as a target list (`[c{:}] = deal(0)`); a cell or a struct cannot be transposed; `isequal` of cells or structs is false; `varargin` with no extra arguments is 0x0; several message texts are recalled or SplatCrab's own. The Design notes of `docs/modules/07-cells-and-structs.md` have each | later (verify first) |
| `exist` gives `5` for every builtin, where MATLAB gives `2` for the builtins it ships as `.m` files (`linspace`, for instance): every SplatCrab builtin is built in. What `exist` gives for a function local to the running script is not settled by the MathWorks page; SplatCrab gives `0`, and no case asserts it | by design; the local-function value later (verify first) |
| The trace names a function alone, `  in g3 (line 8)`, where MATLAB writes `Error in script>g3 (line 8)`; the spec fixes SplatCrab's form | by design |
| Functions are more permissive than MATLAB's in three ways, none of which changes what a file MATLAB accepts means: a function in a file on the path can call a local function of the script being run (invariant 4 reads the script's local functions after the running file's, where MATLAB keeps local functions private to their file); a file may mix functions that end with `end` and functions that do not; and a script's local functions may go without `end`. Calling a script with arguments or for a value is `Too many input arguments.` or `Too many output arguments.`, where MATLAB names the script | later |
| Command syntax judges "is a variable" when the source is lexed, from the workspace and the names assigned earlier in the source, so `x = 1; clear x; x -1` stays the expression; MATLAB judges a file the same way, the command line from the live workspace | by design; see cycle 04's Design notes |
| Function handles, cycle 06: `func2str` renders an anonymous function from its parse tree, so `@(x) (x)` reads back `@(x)x` where MATLAB keeps the text as written; the trace names an anonymous function `  in @(n)g(n)` with no line; an anonymous call counts against the recursion limit of 500; `str2func` of a text that is not a name makes a handle that fails only when called. The Design notes of `docs/modules/06-function-handles.md` have each | by design (verify first) |
| Strings and files, cycle 11: `delete` refuses a wildcard rather than expand it; the regular-expression engine refuses backreferences, lookaround, atomic groups, possessive quantifiers, conditionals and inline flags, since it runs in linear time; `str2num` reads literals and operators only, where MATLAB hands its text to `eval`; `input` of text that is not an expression is an error, where MATLAB asks again; `feof` is set by a read that ends at the end of the file; a compressed MAT-file is refused, and the integer and `single` classes load as doubles; a struct array with no fields past 1,048,576 elements is refused by `load` and `save`, since no bytes of the file bound it; `save` in an empty workspace is an error rather than a file of a header alone, which `load` would refuse; `fopen` takes no machine format or encoding, and text is UTF-8 both ways; several message texts are SplatCrab's own. The Design notes of `docs/modules/11-strings-and-io.md` have each | by design (verify first) |
| `warning('off')`, `warning('on')` and `lastwarn` do not exist: `warning('off')` prints `Warning: off` | later |
| The environment, cycle 13: `format` has `short` and `long` only; `whos` has no heading row and its layout is SplatCrab's; `ls` and `dir` print one name per line with no `.` or `..`, and `dir` returns no `date` or `datenum`; a bare `pause` waits for Enter rather than any key; `evalc` drops the final line end of what it captured, as the spec records; `datestr` takes date numbers or one date vector and no format; `exit(n)` takes 0 to 255; `which` and `help` do not report local functions. The Design notes of `docs/modules/13-environment.md` have each | by design |
| Plotting, cycle 12: `gcf` and `figure` return the figure's number as a double, as MATLAB did before R2014b, where MATLAB now returns a Figure object whose `disp` lists its properties; there are no graphics objects or handles; a complex argument is refused, where MATLAB plots the real part against the imaginary; labels are plain text, with no TeX; `plot` takes no name-value options; `histogram` with no bin count uses Sturges' rule; the tick rule, the layout, the fonts and the SVG and PNG bytes are SplatCrab's; a line with a `NaN` gap is one `<polyline>` a run. The Design notes of `docs/modules/12-plotting.md` have each | by design |

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
Cycle 07 fixed the row scheduled to it, the builtins that refused an
`MException` and a handle, and narrowed it to `isequal` of handles, which
its spec left out of scope. Cycle 08 fixed the row scheduled to it, `inv`
and `A^-1` of a singular matrix (QA D26), which now warn and return `Inf`,
and with it the Known deviations rows for square-only backslash and
vectors-only `norm`.
Cycle 11 fixed the four rows scheduled to it: `printf`'s conversions and
flags (QA D16), `num2str` of a matrix (QA D13), `fprintf` to a file
identifier and its byte count (QA D25), and the unexpected character
echoed raw into the lexer's message; it also discharged the Known
deviations row for a char range and `diag` of a char.
Cycle 13 fixed the two rows scheduled to it, `clc` writing its escapes into
captured output and `exit` and `quit` working only as bare REPL lines
(QA D28), and discharged the Known deviations row for `who` and `whos`.
Bug-fix pass 13b fixed the three rows left to a bug-fix pass: the hang of an
element-wise operation on an empty array with a huge dimension, the struct
array's memory that the element cap did not bound (now a byte budget for
every cell and struct array), and the non-UTF-8 row, which it closed for
UTF-16, the only half left; other encodings stay out of scope, a leniently
decoded file being read rather than refused.
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
| A colon operand that is not a scalar is an error | `[1 3]:4` is `range start must be a scalar.`, and so therefore is `1:2:3:4`, which cycle 01e taught the parser to read as `(1:2:3):4`. MATLAB is understood to take the first element of a non-scalar colon operand, which would make it `1:4`; that was not verified against a real MATLAB run, so 01e fixed the parse and left the evaluation as it found it. Verify before changing it | later (verify first) |
| Constructors take two sizes only | `zeros(2, 3, 4)` is "N-D arrays are not supported."; MATLAB builds a 2-by-3-by-4 array. The same holds for `ones`, `rand`, `NaN`, `Inf`, `true`, `false`, `reshape` and `repmat`, with separate sizes or a size vector. Since cycle 01c a trailing size of `1` is dropped, as MATLAB drops it, so `zeros(2, 3, 1)` is 2x3, and any other third or later size, `0` included, is that clean error. Before 01c, `zeros`, `ones` and `rand` with three sizes were "Too many input arguments.", and before cycle 01 they built the 2-D array and dropped the third size. The row stays, because building N-D arrays needs a design that no roadmap module claims yet. `eye` is unaffected: MATLAB rejects `eye(r, c, p)` too | later, needs N-D arrays |
| Hex and binary literals are unsupported (QA D30) | `x = 0x1F` is `unexpected 'x1F'`; MATLAB R2019b+ and Octave give `31` | later, low impact |
| `isequal` of handles answers false | `isequal(f, f)` is false for any handle, where MATLAB compares them. Cycle 07 made the shape and class queries answer for every value and left this half of the row, which its spec keeps out of scope until a source settles MATLAB's rule | later (verify first) |
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
