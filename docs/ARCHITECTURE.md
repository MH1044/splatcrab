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
                   ├──► interp.rs           eval, output captured; figures, figure: the
                   │                        figures in memory, as SVG text
                   ├──► syntax.rs           complete: is the entry finished?
                   ├──► env.rs              completions: variables + path files + builtins;
                   │                        workspace's value previews
                   ├──► files.rs            files: one folder under the file root;
                   │                        read_file, write_file, run_file: one file there;
                   │                        cwd: the current folder, judged as files judges
                   └──► history.rs          history, history_add: the shared history file

 browser ──► server.rs ──────────────► http.rs ──► protocol::respond
 127.0.0.1   a reader thread per       limits, Host, Origin, token,
 only        connection (16 at most),  Sec-Fetch-Site, routes
                                       │
             a channel to the one      └──► src/ui/  index.html, app.js, app.css,
             interpreter thread                      embedded with include_str!
 stdin ────► http::serve_stdio (--http-stdio): the same http::handle, no socket
```

`src/lib.rs` exposes the sixteen modules: the six of the language (`lexer`,
`parser`, `interp`, `value`, `builtins`, `error`), the four of the
evaluation protocol that cycle U0 added (`json`, `syntax`, `env`,
`protocol`), the two of the UI server that cycle U1 added (`http`,
`server`), cycle 12's `plot`, cycle 13's `editor` and `history`, and cycle
U2's `files`. `src/ui/` holds the page's three files, which `http.rs` embeds.
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

Since cycle 16 a number that starts with `0` straight followed by `x`, `X`,
`b` or `B` is a hexadecimal or binary literal, read in a branch of its own
just before the number branch: `0x2A`, `0b101010`, `0xFFs8`. The literal is
the prefix and the whole run of letters, digits and underscores after it,
which must be one or more digits of the base and then at most one of the
eight suffixes `u8` to `u64` and `s8` to `s64` (`INT_SUFFIXES`).
`radix_literal` finds the run and `radix_value` its value, computed exactly
in a `u128` that stops accumulating once the value passes 2^64 - 1, so a
literal of any length is read in one pass and never overflows. The value
must fit its type, below 2^64 with no suffix and below 2^N with `uN` or
`sN`, and with `sN` a value of 2^(N-1) or more is its two's complement,
`0xFFs8` being `-1`. It is one `Token::Num` holding the nearest double, so
every consumer of a number takes it as it takes any other; the one number
token that can be negative, a signed literal with its top bit set, becomes
the negation of its magnitude in the parser (below). A malformed run,
`0x`, `0b102`, `0x1Fz`, `0x100u8`, is `invalid number '<text>'` naming the
whole run as written, at its line. Every other number is read as before,
so `00x1F` and `1x2` are still a number followed by a name.

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

Since cycle 16 a negative number token, which only a signed hexadecimal or
binary literal with its top bit set makes (`0xFFs8`), is parsed as the
negation of its magnitude, `Expr::Neg(Expr::Num(1))`, the tree `-1` written
in decimal makes, and counts as one level of nesting as that sign does. The
value is the same double, and `render` gives it a negation's precedence:
`@() 0xFFs8^2` renders `@()(-1)^2`, where a bare `Expr::Num(-1)` rendered
`@()-1^2`, which reads back as `-(1^2)`. So every `Expr::Num` the parser
makes holds a number that is not negative, and a rendered body still parses
back to the same tree.

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
`data[c * rows + r]`, and since cycle 14 element `(i1, i2, ..., ik)` of an
N-D array, zero-based, at `i1 + d1*(i2 + d2*(i3 + ...))`. This is not an
implementation detail. It is what makes `A(:)`, `reshape`, and linear
indexing produce MATLAB's answers, and every new operation must respect it.
A `Matrix` keeps its `rows` and `cols` and, beside them, the dimensions
past the second in a private `higher`, stored normalised: a trailing
dimension of 1 is never stored, so a 2-D matrix has an empty `higher` and
is exactly what it was before cycle 14, and every 2-D kernel keeps
compiling and stays correct. `Matrix::dims` answers every dimension,
`ndims` is 2 plus the stored count, and `from_dims`, `filled_dims` and
`set_dims` are the only ways to make or change an N-D shape, each
normalising it; see "Add a value type". Since cycle 14b `along_dim` sees
any shape along one dimension as three numbers, `[before, n, after]`,
element `(b, k, a)` of the view at `b + before * (k + n * a)`: the one view
the reductions, the running scans, `cat` and the brackets, and `repmat`
work through, whatever the array's `ndims`, and since cycle 14c `sort`,
`numerics::map_slices` (and so `diff`), the flips and `circshift`. Every `Matrix` carries a `class`
tag, `Double`, `Logical` or `Char`, over the same `f64` storage; a char
element is one UTF-16 code unit. Since cycle 10 a double can be complex: `im: Option<Vec<f64>>`
holds the imaginary parts, column-major like `data`, and is `None` for real
storage; see "Add a value type". `value.rs` also owns the display: `format` for the numeric body,
`disp_text` for `disp`, and `display_body` for the class headers of a named
display, and since cycle 14 `page_display` for the pages of an N-D array,
which `write_pages` hands to the output sink a page at a time.
Since cycle 04 `Value` has a second variant, `Exception`, the
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
generation counter keeps honest, and from which `file_written` drops the
parse of a file the interpreter writes, and every lookup, since cycle U3. `run` runs a script, local functions allowed; `run_command` is
the REPL's and the protocol's, and refuses a definition; `run_file`
(cycle U3) is the protocol's `run_file`, a file run as a command-line
entry of `run` with its full path. It no longer knows what any individual
builtin does.

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
amortised. Since cycle 14 the pipeline takes every dimension: the resolvers
take the array's dims, `fold_dims` folds the dimensions past the last
subscript into it or pads with ones, `end_value` is `end` in each position
(the dimension, or in the last position the product of the rest), a read
has one dimension per subscript, a growth may add pages and dimensions,
judged by `check_dims`, `regrid` lays the old elements out in the grown
shape (resizing in place when `keeps_layout` says the layout does not
move), and a deletion removes positions along any one dimension. Cells and
structs go through it with two dimensions and refuse, through `flat`, any
result that would make one N-D. Since cycle 14c a read or a write through
fewer subscripts than dimensions judges its shape from `fold_asked`, the
fold as asked in `f64`, which is `fold_dims`'s wherever the product fits
a `usize` and the product itself where an empty array's sizes pass it, so
`x(:, :)` of `zeros(0, 2^40, 2^40)` is `Requested 0x1.20893e+24 array
exceeds the maximum array size.`, never the saturated `1.84467e+19`.

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
along a dimension: it hands each slice to a closure, and judges the
result's shape first; since cycle 14c it runs over the three-number view
of any array along any dimension, a matrix along 1 or 2 read column by
column or row by row in the order the columns and the transposed rows
always were, and a result that would have more dimensions than
`args::MAX_NDIMS` is refused before its list of sizes is made. `first_dim`
is MATLAB's default dimension, since cycle 14b the reductions'
`math::default_dim`, so the two cannot disagree. `sets.rs` holds the five
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
refuses one to any other; see "Add a value type". Cycle 14 added the same
gate for N-D arrays beside it, `builtins::ND_OK` and `nd_gate`, and cycles
14b and 14c grew `ND_OK` by the builtins they taught N-D. Cycle 14c put
`ipermute`, `horzcat` and `vertcat` on `TAKES_COMPLEX`, since they keep
complex storage as `permute` and `cat` do; `flip` and `circshift` stay
off it, and refuse a complex argument as `fliplr` does.

**The reductions (cycle 14b)** are one kernel, `math::each_slice`, over
the array seen along the dimension as `[before, n, after]`: `f` of each of
the `before * after` slices, in the result's column-major order. The
`sum` page's three sentences are its rule for every reduction, `sum`,
`prod`, `mean`, `any`, `all`, `max`, `min`, and the scans `cumsum` and
`cumprod`: with no dimension the first whose size is not 1
(`math::default_dim`), the 2-D 0x0 keeping its special cases; along `dim`
that dimension becomes 1 and every other stays, the shape judged by
`check_dims`; along a dimension of size 1 within `ndims` each slice is
one element, and along one of size 0 each is empty. Past `ndims` no
kernel runs: `reduce`, `reduce_c` and `scan` hand the argument back
(`math::past_ndims`), the path a matrix and a dimension past 2 always
took, values and storage alike, so `sum` returns `A`, a `-0` stays `-0`,
and `prod(complex(1, 0), 3)` keeps its complex storage. Since cycle 15
`sum` and `mean` take that path along a dimension of size 1 within `ndims`
too, the default dimension judged the same way (`math::reduce_or_return`,
by the `sum` and `mean` pages' "or when `size(A,dim)` is 1"), so
`sum(-0)` is `-0`, while `prod`, `max`, `min`, `cumsum` and `cumprod`
reduce each element on its own there; and past `ndims` `any` and `all`
give each element what they give it along a dimension of size 1, a `NaN`
ignored by `any` and nonzero to `all`, where they converted `A` to a
logical and refused a `NaN`. The statistics of `numerics.rs` keep their
own rules past `ndims` (cycle 14c, by their pages): `median` and `mode`
reduce each element on its own there (`numerics::reduce_along`), which is
`A` itself as a double and a `mode` frequency of 1, 0 for a `NaN`, and
`std` and `var` give "an array of zeros the same size as `A`"
(`numerics::deviation`), a `NaN` or an `Inf` element included, where each
element's own variance made `var(NaN, 0, 3)` a `NaN`. `max` and `min` give
an empty along a dimension of
size 0, their index is the same kernel over `arg_extremum`, and past
`ndims` it is all 1s. A 2-D matrix within its two dimensions is the case
`before = 1` (a column, contiguous) or `after = 1` (a row), with each
slice's elements in the order they always were read, so every 2-D answer
is the one it was.

**Search, sort and the slice functions (cycle 14c)** take any array
along any dimension through the same three-number view: `sort` sorts each
of its slices on its own, stably, along the first dimension whose size is
not 1 or along `dim`, handing the argument back past `ndims` with every
index 1; `diff` goes through `numerics::map_slices`, each of its rounds
along the first dimension that is not 1 of what the round before left,
or every round along `dim`, the result empty along a dimension past
`ndims`, where it was the N-D refusal; `median`, `std`, `var` and `mode`
are the reductions' kernel within `ndims`; `fliplr`, `flipud` and `flip`
reverse each slice along dimension 2, 1 or the one asked for, a page at a
time; and `circshift` rotates each block of `before * n` elements by
`s * before`, `s` the shift taken modulo the dimension's size in `f64`.
`find` of an N-D array is a column of linear indices, and its second
output the linear index over the dimensions past the first, which
column-major storage makes the linear index divided by the rows; by the
`find` page's convention a scalar zero, `find(0)` and `find(false)`, gives
`[]` for every output, where it was 1x0. `ipermute` is `permute` by the
inverse order, its order judged by `permute`'s rule in its own name, and
`horzcat` and `vertcat` are `cat` along 2 and 1, through `interp::concat`,
so their empties follow `cat`'s rule and not the brackets'. `sub2ind` and
`ind2sub` convert by the index pipeline's own fold: the last subscript of
`sub2ind` runs over the product of the sizes from its own on, and the
last output of `ind2sub` is unbounded, as the `ind2sub` page's example
shows.

Cycle 11 added five files. `printf.rs` is the formatter the `printf`
family shares, moved out of `core.rs` and finished (QA D16): `%x`, `%X`,
`%o`, `*` fields, the `#` flag, the remaining escapes and MATLAB's rule
that an invalid conversion ends the output. Every width and precision,
written or given by `*`, is still judged against `printf::MAX_FIELD`
before it reaches Rust's formatter, whose panics it prevents.
`strings.rs` holds the string functions, `num2str` among them (one row
per matrix row since QA D13); since cycle 15 the places whose pages take
a character vector read it through `vector_units` (`Matrix::is_char_row`)
and refuse a char of several rows with the message they give any value
that is not text, where each read one as a single row of its code units:
the three arguments of `strrep`, the two of `strfind`, the text, the
expression and the replacement of `regexp` and `regexprep`, the text and
the delimiter of `strjoin` and `strsplit`, the text of `strtok`, and the
elements of a cell given to any of these or to `upper`, `lower` and
`strcat`. Everywhere else a char is read as before: `upper`, `lower` and
`strcat` of a bare char array, `strtok`'s delimiters, the option names of
`regexp`, `regexprep` and `strsplit`, the `strcmp` family, `str2double`
and the number conversions; and `strtrim` trims a cell's element of
several rows as it trims that array alone. It holds `regexp` and
`regexprep` over
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
function that writes or deletes a file names it to `Interp::file_written`
(`fopen` for writing and `fclose` after a write through
`file_written_as`, sharing one `FileId` held on the open file), so a `.m`
file a script writes is found, and run from its new text, by the next
call. `Interp.input` is
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
figure inline without a file. Since cycle U4 the protocol's `figures`
answers the open figures and those `Interp::take_changed_figures` names,
and `figure` one figure's SVG text, at most 32 MiB of it, which the
browser page shows inline under the entry that changed it, as an `<img>`
of a `blob:` URL, never as markup. A
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
`completions` and the terminal's Tab both call it (cycle 13). Since cycle
U2 `preview(value, format)` is the workspace pane's text for a value, which
`workspace` with `"preview": true` answers: a small numeric or logical
array written out element by element as `disp` prints each alone, a
one-row char array quoted as a literal, a handle's text, or else the size
and class, cut at 80 scalar values. It costs what it shows: at most 10
elements are displayed, a char row is decoded lazily, only as far as the
cut can keep, and a handle's text is rendered under a budget of one
character past the cut (`Func::shown_up_to`), never whole.

**`files.rs`** (cycle U2) is the file browser's listing: `list(root, path,
bound)` lists one folder under the file root by the confinement rule its
module comment records (the text refused or resolved first, with no disk
access, then the canonical path judged inside the root before its kind),
folders first and each group in byte order of name, at most `bound`
entries, kept in a heap of that size so a huge folder costs a look at each
name, and one resolution of each entry that is a link, and no more. A link
is listed by its target only when that is inside the root. `session_root` is the root a client mode fixes as it starts.
Since cycle U3 it holds the editor's three operations too, by the same
rule's first two steps: `read_file(root, path, bound)` reads one regular
file inside the root, its length judged from its metadata before a byte
is read and the read taken no further than the bound plus one, and its
bytes UTF-8; `write_file(root, path, text, bound)` judges the name (no
Windows device name, no trailing `.` or space, on every platform), then
the folder's canonical path, then whatever is at the name, a link
included, by where it leads, then the text's length, and never creates a
folder; `run_file(root, path)` judges a `.m` file as `read_file` does and
names the path to run it by, on Windows the plain form of the verbatim
root's when that names the same file. `relative(root, file)` names an
error frame's file relative to the root, judged on its canonical path.
The bound is `MAX_TEXT`, 4 MiB, a parameter so a unit test reaches it.
Since cycle U4 `current_folder(root, path)` judges the folder the
protocol's `cwd` moves to exactly as `list` judges one, and names it as
`run_file` names a file, the root joined with the components in one push,
in its plain form; `folder_relative(root, dir)` answers the current folder
relative to the root, `""` for the root itself.

**`editor.rs`** (cycle 13) is the terminal's line editor as a pure state
machine: `Decoder` turns the characters a terminal sends, ANSI escape
sequences included, into `Key`s, and `Editor::key` turns a key into a new
buffer, cursor and history place, or an `Action` (submit, cleared by
Ctrl-C, end of input, a list of completions), asking a completion function
it is handed for Tab. It reads and writes nothing, so every key is unit
tested. **`history.rs`** is the history file: UTF-8, one entry per line,
oldest first, with `\`, `\n` and `\r` escaped so a multi-line entry stays
one line; appended to one entry at a time and compacted on load past twice
its 1000 entries, at `SPLATCRAB_HISTORY` or `~/.splatcrab_history`. Since
cycle U2 the protocol's `history` and `history_add` read and extend the same
file through the same functions, so the browser's history pane and the
terminal share one history. Since cycle 17 it is bounded in bytes twice:
`remember` refuses an entry of more than `MAX_ENTRY_BYTES` (64 KiB) of
UTF-8, so neither the line editor nor `history_add` keeps or appends one;
and `load` judges the file's length from its metadata and reads at most
its last `MAX_FILE_BYTES` (4 MiB), seeking there and dropping the partial
line the read starts in, leaves out an entry past `MAX_ENTRY_BYTES` that
another program wrote, and compacts a longer file to the entries it kept,
as it compacts one past twice its entries. Both bounds are parameters of
`load_within` and `remember_within`, so a unit test reaches them with
small numbers. **`src/term.rs`**, a module of the
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
only at end of input. The message texts live in `error.rs`. Since cycle U3
`read_file`, `write_file` and `run_file` work on one file under the file
root through `files.rs`, `run_file` capturing its run as `eval` does and
`write_file` calling `Interp::file_written` with the file it wrote, and an
error object carries its `stack`, the frames innermost first, on `eval`
when the request's `stack` is `true` and always on `run_file`. Since cycle
U4 `figures` answers the open figures and those changed since it was last
asked, `figure` one figure's SVG text, the text `saveas` writes, refused
past `MAX_INLINE_SVG` (32 MiB, a parameter of the function so a unit test
reaches it), and `cwd` the current folder relative to the root, after
moving it, with `path`, to a folder judged as `files` judges one, through
`Interp::enter_folder`, which `cd` uses too. All three are asked for, as
`workspace` and `history` are, since `eval`'s bytes are U0's.

**`http.rs`** is the UI server's HTTP, as a pure function:
`handle(request_bytes, &mut Interp, &Config) -> Vec<u8>`, with `Config`
holding the port and the token. It parses the head (request line, headers
with case-insensitive names, CRLF or bare LF), refuses what it must with
`400`, `403`, `404`, `405`, `413`, `415`, `431` or `501`, serves the three
embedded files, and hands the body of a `POST /api` that passed every check
to `protocol::respond`. Responses are built in one place, so the header
order and a `Content-Length` equal to the body's length hold for all of
them; the status texts are a table in `error.rs`. Since cycle U3 a `POST
/api` whose `Sec-Fetch-Site` is present and not `same-origin` is `403`,
judged after the token and before the content type. `read_request` frames one
request off any `BufRead` under the head and body caps, shared by the socket
and by `serve_stdio`, the `--http-stdio` loop the golden cases drive.

**`server.rs`** is the socket around it: `bind` to `127.0.0.1`, the session
token, the browser launch, and `serve`. Since cycle U2 a thread of its own
accepts, and each connection is read on a thread of its own, at most
`MAX_CONNECTIONS` (16) at once, a connection past that closed at once with
no answer: the thread reads one request under a 10-second deadline for the
whole of it, so a client trickling a byte at a time holds only its own
thread, and hands the bytes through an `mpsc` channel to the interpreter
thread, which runs `http::handle` one request at a time in the order they
arrived whole and sends each answer back; the connection's thread writes
it under its own deadline, lingers and closes. The `Interp` never leaves
the thread that called `serve`, which it could not, since it holds `Rc`s.
It holds no policy: every check is in `http.rs`, where a unit test can
reach it.

**`src/ui/`** is the page: `index.html`, `app.js` and `app.css`, embedded
with `include_str!` so the binary is the whole program, with no framework
and no external resource. Since cycle U2 it is a desktop of four panes, the
file browser, the command window, the workspace and the command history,
with three splitters whose sizes are CSS custom properties set from script.
Every request goes through one promise queue in `app.js`, one at a time,
and everything a request returns reaches the page as text, never as
markup. The palette is defined once, as custom properties of the light and
the dark `:root` rules, which a unit test in `http.rs` enforces. Since
cycle U3 an editor sits above the command window, behind a fourth
splitter: tabs of open files, a line-number gutter beside a `<textarea>`
that does not wrap, Save, Run (`run_file`) and Run Selection (an `eval`),
its questions asked in the page and never in a browser dialog; every
`eval` the page makes asks for the stack, whose frames the command window
lists under an error, a frame with a file being a link that opens it.
Since cycle U4 every entry, typed, recalled, Run or Run Selection, is
followed by `figures` and a `figure` for each figure it changed, each
shown under the entry's output as an `<img>` whose source is a `blob:` URL
the script makes from the SVG text and revokes once it has loaded, a
snapshot a later entry does not change; entries run one at a time, each
after the one before has shown its figures. Tab in the command window
completes the name before the cursor through `completions`, or inside a
quoted string a file or folder name of the current folder through `cwd`
and `files`, several listed under the prompt until the next key. The file
browser is the current folder: it lists what `cwd` answers, and a click
on a folder or on `..` moves the current folder there with `cwd`.

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
   points it at the same capture as `out`. Since cycle 14 the body of
   `emit` is `emit_to`, which `show_var` calls directly, since it holds
   the variable it displays borrowed where it is stored while it writes
   the display a page at a time; it writes to the same sink.
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
   client can make it allocate without bound; since cycle U2 it reads at
   most 16 connections at once, `files` judges a path in time linear in its
   length and lists at most 10,000 entries of a folder, keeping no more
   than that many in memory however large the folder, and a workspace
   preview displays at most 10 elements, and decodes a char row and
   renders a handle's text only as far as its 80-character cut; since
   cycle U3 `read_file` judges a file's length before reading a byte and
   reads at most 4 MiB and one byte of it, `write_file` judges the text's
   length before writing, and neither opens anything but a regular file,
   so no device or pipe can make one wait; since cycle U4 `figure` answers
   at most 32 MiB of SVG text, a figure's render being bounded by the
   point budget below, and `cwd` judges its path as `files` does; since
   cycle 17 the history file is bounded in bytes: an entry of more than
   `history::MAX_ENTRY_BYTES` (64 KiB) is neither kept nor appended, and
   `history::load`, which `history` and `history_add` call and the line
   editor calls as it starts, judges the file's length from its metadata
   and reads at most its last `history::MAX_FILE_BYTES` (4 MiB) and the
   byte before them, never the rest, so a `history` answer carries at most
   1000 entries decoded from 4 MiB of the file, however large the file
   has grown, and a longer file is compacted to them. Anything
   that computes a result shape from its
   operands' shapes goes through `args::check_shape`, or `args::check_dims`
   for any number of dimensions (cycle 14), for the same reason; see the
   recipe below. An iteration that has no fixed trip count has a cap
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
   every array's dimensions through `check_shape` before it allocates
   (since cycle 14b `check_dims`, every dimension of the array's
   dimensions array, which is itself read from the bytes there),
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
   written one. Since cycle 14 no width a program controls reaches Rust's
   formatter at all, which panics on a runtime width past 65,535: `whos`
   and a struct's field lines pad their columns by hand
   (`value::push_left`, `push_right`), since a size text or a name has no
   length limit; and the display of an N-D array is written a page at a
   time, so its memory is one page's whatever the page count. Since cycle
   15 `isequal` walks nested cells and structs with a worklist of pairs,
   never by recursion, so a `c = {c}` chain a million deep compares
   without touching the stack; a pair of containers that more than one
   path reaches is compared once, so a nest of few shared containers and
   exponentially many paths compares in time proportional to the pairs of
   containers it compares, never to its paths; and struct fields are matched through the struct's own
   field index, so fields in another order are never matched in quadratic
   time. `strtok`, whose delimiters may be of any size, looks each unit of
   its text up in a set of them, so a long text against a long delimiter
   list costs time linear in both. Since cycle 14c `circshift` takes each
   shift modulo its dimension's size, exactly in `f64`, so any integer a
   double holds costs one pass per dimension that moves (at most log2 of
   the element count, since only a dimension of 2 or more moves), and an
   entry of a vector shift past `ndims` moves nothing and sizes nothing;
   `sub2ind` judges and weighs each scalar subscript once, not once per
   position, and `ind2sub` does no more work than the outputs it writes,
   so both run in time linear in their inputs and outputs; since a program
   can ask for as many outputs as the targets it builds and evaluates,
   `ind2sub` judges its outputs together before writing any, their
   elements `numel(ind)` by `k` and their lists of sizes `ndims(ind)` by
   `k` through `check_shape`, so no count of outputs makes it write more
   than one array may hold, and `arrayfun` of an N-D array judges its
   uniform outputs' lists of sizes, `ndims` by the outputs, before it calls
   `f`, though not their elements times the outputs, which `deal` and
   `arrayfun` and `cellfun` of matrices do not bound either (the Known bugs
   row on a program's memory); and nothing a
   slice function does walks a dimension of size 1 per element: `sort`,
   `flip` and `circshift` pass over a dimension of size 1 without a loop,
   and the three-number view costs one product over the dimensions
   however many singletons an array holds.

## Recipes

### Add a builtin

1. Write the function in the right file under `src/builtins/`: `core.rs` for
   constants, constructors, shape queries, output, the workspace and timing;
   `math.rs` for element-wise and reducing numerics; `linalg.rs` for linear
   algebra, rearrangement, search and sort, and since cycle 14c the index
   conversions `sub2ind` and `ind2sub`, with the numerics of a
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
7. If it takes an N-D array (cycle 14), handle every dimension, reading
   `Matrix::dims` rather than `rows` and `cols`, and add its name to
   `builtins::ND_OK`. Otherwise do nothing: the registry refuses an N-D
   argument to it before it runs, so it can read `rows x cols` as the
   whole array.

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
it is judged, so that an oversized request is named as asked. `shape_dims`
reads a constructor's sizes (cycle 14): none (1x1), a scalar `n` (n x n), a
row size vector, or two or more scalars, with trailing sizes of `1` dropped
and every other size kept, so `zeros(2, 3, 4)` asks for 2x3x4. `shape` is
the two-size form that `eye` and `cell` keep (`repmat` reads
`shape_dims` since cycle 14b), any other third
size its N-D error. `size_list` and `trailing_ones` are their lower layers,
for a builtin such as `reshape` that takes a `[]` placeholder or has no
n-by-n rule. `size_arg` and `size_value` read one size as an `f64`, where a
negative size is `0`. `check_dims(&[f64])`, and `check_shape(rows, cols)`,
which is it for two sizes, are the only sanctioned ways to turn a requested
shape into allocation lengths: they judge the product of every size against
`MAX_ELEMS` and refuse naming every size. Indexed growth uses them too since
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
`Sel::covers`), and since cycle 14b the reductions' kernel, `permute`,
`cat` and every bracket with an N-D operand through `interp::concat`, and
`repmat`'s tiling, and since cycle 14c `sort`, `find`, `diff`, the
statistics, `flip`, `fliplr`, `flipud`, `circshift`, `ipermute`,
`horzcat`, `vertcat`, `sub2ind`, `ind2sub` and `arrayfun` along any
dimension of any array, an empty one returned at once however large its
other sizes (`sort(zeros(0, 1e6, 1e6), 3)`),
loop over the elements they produce or read, never over a
dimension of an empty operand, so `zeros(0, 1e12) + 1` returns at once and
`x(:, :)` of it lists no positions. `for` over an array with no rows still
iterates its columns, which is the open question of its own Known bugs row.
A walk over an N-D shape passes over its dimensions of 1 (cycle 14):
broadcasting's counter, the positions several subscripts select
(`sel_positions`) and `regrid`'s runs, so a step costs the same however
many singletons the shape holds, and `zeros([ones(1, 1e5) 2000]) + zeros(2,
1)` takes the time of its 4,000 elements rather than that times 100,000.

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
true. Cycle 14's go through `check_dims`: the N-D constructors and
`reshape`, broadcasting across every dimension, a read's shape of one
dimension per subscript, and a growth's shape and its count of positions,
which repeated subscripts can make far larger than the array. Cycle
14b's too: a reduction's shape, a concatenation's (`cat`'s and every
bracket's with an N-D operand), `repmat`'s whole shape, named as asked
and never an intermediate, and every dimension a MAT-file declares.
`check_dims` also refuses a shape of more than 2^20 dimensions
(`args::MAX_NDIMS`) before it judges the sizes, `Arrays have at most
1048576 dimensions.`, and when `cat`'s dimension passes every argument's
`ndims`, it is judged against the same bound before the result's list of
sizes is made, so `cat(1e10, 1, 2)` is a clean refusal rather than a
request for 1e10 sizes. Cycle 14c's too: `map_slices`'s shape, and so
`diff`'s, whose `dim` past `ndims` gives the result that many dimensions
and is judged against `args::MAX_NDIMS` before any list of sizes is made
(`diff(1:3, 1, 1e10)` is the refusal at once, and `diff(X, 0, 1e300)` is
`X`, making no dimension), the statistics' through the reductions' kernel,
and `horzcat`'s and `vertcat`'s through `cat`'s; and the read through
fewer subscripts than dimensions, whose fold is judged as asked in `f64`
(`fold_asked`). The others of cycle 14c make no shape larger than an
argument's: `sort`, the flips and `circshift` keep their argument's shape,
`ipermute` permutes it, `sub2ind` answers in its subscripts' shape,
`ind2sub` in its index's and `arrayfun` in its arrays'. A new operation of
that kind belongs on the same list.

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

Cycle 14 gave `Matrix` its dimensions past the second the same way, as a
field beside `rows` and `cols` rather than a dimension vector in their
place (see `value.rs` above), and guards them the same way. A 2-D kernel
reading `rows x cols` of an N-D array would read its first page alone in
silence, so the registry refuses an N-D argument to every builtin not on
`builtins::ND_OK` with `N-D arrays are not supported by '<name>'.`, through
`nd_gate`, which `Interp::call_builtin` runs beside `complex_gate`; that is
the only path into a builtin. Since cycle 14b `ND_OK` holds, beside cycle
14's constructors, shape and class queries, `isequal`, `reshape`, `disp`,
conversions, printf family, `feval` and `deal`, the reductions (`sum`,
`prod`, `mean`, `any`, `all`, `max`, `min`, `cumsum`, `cumprod`), the
element-wise math and the two-argument functions that broadcast, and
`squeeze`, `permute`, `cat` and `repmat`, and since cycle 14c `sort`,
`find`, `diff`, `median`, `std`, `var`, `mode`, `fliplr`, `flipud`,
`arrayfun`, `flip`, `circshift`, `ipermute`, `horzcat`, `vertcat`,
`sub2ind` and `ind2sub`, and nothing else: `num2str`, `mat2str`, the
strings, the sets, the linear algebra, `trapz`, `cumtrapz`, `filter`,
`cellfun` and every other builtin not named still refuse an N-D argument.
`arrayfun` with `'UniformOutput', false` refuses one itself, `N-D arrays
are not supported.`, since its cell of results would be N-D. The interpreter's own consumers, which no
gate covers, each handle every dimension or refuse: the index pipeline,
the element-wise primitives (`map`, `try_map`, `map_c`, `zip`, `try_zip`,
`zip_c`, `to_class`, `real_part`, `imag_part`, each carrying every
dimension, the zips broadcasting across them), the operators, which refuse
an N-D operand where they are not element-wise (`Matrix operations are not
defined for N-D arrays.`), `'` and `.'` (`Transpose is not defined for N-D
arrays.`), the brackets, which join one along any dimension through the
kernel `cat` uses (cycle 14b), `for` over the 2-D fold, the display,
`whos`, the protocol's `workspace`, the preview, the cell and struct
summaries, `values_equal`, which compares every dimension before any
element, and `save`, whose MAT-file writer writes one's whole dimensions
array wherever it sits (cycle 14b; cycle 14 refused one), refusing a
dimension past 2147483647, which its `int32` words cannot hold, before
anything is written (`Unable to save variable 'C': a dimension past
2147483647 cannot be written in a MAT-file of version 5.`), and whose
`-ascii` text, which has rows alone, refuses one. `Value::dims`
answers every dimension and `Value::numel` their product, and
`Matrix::is_vector` is false for an N-D array. Cells and structs stay 2-D:
an element or a field may hold an N-D array, but a cell or struct array is
never one. The gate judges a matrix argument only, so a builtin that reads
a cell judges an N-D element itself: `cell2mat` through the bracket rule,
`save` in its writer, the set functions and `str2double`, for which an
N-D char is no character vector however few its rows
(`Matrix::is_char_row`), `strcmp` and `strcmpi`, which compare one by
every dimension, not by its rows and columns alone, and since cycle 15 the
text functions that take a cell of character vectors, which refuse one as
no character vector.

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
`feval`, `arrayfun`, `func2str`, `class` and `isa`, and since cycle 15
`isequal`, by the MathWorks "Compare Function Handles" page: two named
handles are equal when they have the same name and the same `local`
binding, compared by pointer, and an anonymous one only to its copies, the
same `Rc<Func>`; `hcat` refuses it with the concatenation message rather
than the generic one, and `'` and `.'` keep the generic refusal.

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
`Value::mat` and keeps the generic refusal, apart from two that cycle 15
taught the containers. `'`, `.'` and `transpose` of a cell or a struct
array (`value::transpose_cell` and `transpose_struct`) interchange the row
and column of every element, each element unchanged. `isequal` compares
two cells, or two struct arrays with their fields matched by name in any
order through the struct's own field index, element by element with a
worklist of pairs rather than by recursion, so a nesting of any depth is
compared in time proportional to its elements (`core::values_equal`).

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
and at the REPL alike. Since cycle 07 an
entry also records the file of the function (`MError::leaving_file`, empty
for a function local to the code that was run), and `e.stack` reads the
entries as an Nx1 struct array of `file`, `name` and `line`. The protocol
sends the entries since cycle U3, as an error's `stack`, on `eval` when the
request asks and always on `run_file`, each file relative to the file root
or `null`; without the flag an `eval`'s answer is U0's, byte for byte.

**Registry (cycle 01, in place).** `BuiltinFn = fn(&mut Interp, &[Value], usize) -> R<Vec<Value>>`,
where the `usize` is `nargout`. An empty `Vec` means the builtin produced no
value: legal at statement level, "Too many output arguments." in an expression.
Copy the function pointer out of the map before calling it, or the borrow
checker will object to `&self` and `&mut self` at once. `Stmt::Expr` asks for
0 values, `eval` asks for 1, and since cycle 03 `Stmt::MultiAssign` asks for
one per target, `~` included. A builtin returns as many values as it has, up
to `nargout`; the caller, not the builtin, turns too few into "Too many output
arguments.", so a builtin with one output needs no change to be asked for two.
`max`, `min`, `sort`, `size` and `find` give more than one, and since
cycle 14c `ind2sub` gives as many as it is asked for.

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
- A `Sec-Fetch-Site` header, which a browser adds to every request a page
  makes, must be `same-origin` when present (cycle U3): a request another
  site makes is refused even if it somehow carried the token. A client
  that sends none, a golden case or a script, is unaffected, and the
  static routes are not checked, since a navigation is not same-origin.
- A refused request never reaches the interpreter: only the last arm of
  `handle` calls `protocol::respond`, after every check has passed.
- The page's `Content-Security-Policy: default-src 'self'; img-src 'self'
  blob:; frame-ancestors 'none'` forbids inline script, anything from
  another origin, and framing. Cycle U4 added `img-src 'self' blob:`, the
  one change to U1's headers: a figure is shown as an image of a `blob:`
  URL, which only the page's own script can make, and script is still
  `/app.js` alone, so the SVG text is never markup and nothing in it runs.

Since cycle U2 the token guards the file system and the history too:
`files` lists folders under the root and `history` returns what was typed
in any session, its newest 1000 entries (since cycle 17 those in the
file's last 4 MiB, each at most 64 KiB). Each connection is read on a
thread of its own, at most 16 at once, and only whole requests reach the
interpreter thread, so an idle connection can no longer hold the
interpreter. Since cycle U3 it guards the first operation that changes
the file system, `write_file`, which can replace any file under the root,
a `.m` file the next `run` executes included; `eval` could already do as
much through `fopen`, so `Sec-Fetch-Site` is one more check on top of the
token, and neither may be relaxed.

HTTP stays minimal: `Connection: close` on every response, no keep-alive, no
chunked bodies, no `Expect: 100-continue`. The same `handle` is driven from
stdin by `--http-stdio`, so the golden cases pin every byte without a socket,
and `tests/ui_server.rs` covers the socket itself. The Design notes of
`docs/modules/U1-ui-server.md` have the details.

**The file root (cycle U2, in place).** Every client mode, `--protocol`,
`--ui` and `--http-stdio`, fixes `Interp::file_root` once as it starts, to
the process's working directory canonicalised, and nothing changes it
afterwards: `files` lists relative to it, whatever `cd` does to
`Interp::cwd`. A path is judged on its text first, a `..` past the root
refused before anything on disk is touched, and then by its canonical form,
which catches links and junctions, judged inside the root before its kind
so a refusal says nothing about what lies outside. The token guards it as
it guards `eval`. The Design notes of `docs/modules/U2-ui-desktop.md` have
the details. Since cycle U4 the file browser is the current folder, asked
for with the protocol's `cwd`, which also moves it; the root itself still
never moves. Since cycle U3 the
editor's `read_file`, `write_file` and `run_file` are confined by the same
rule, `write_file` judging the name, the folder and whatever is at the
name before it writes, so it never follows a link out of the root, and
`run_file` runs from the root wherever `cd` has gone; the Design notes of
`docs/modules/U3-ui-editor.md` have the details.

**The current folder (cycle 13, in place).** `Interp::cwd` is the one
current folder: every path lookup, every file builtin, `ls`, `dir`, `run`
and `system` resolve against it, and `cd` changes it through
`Interp::set_cwd`, which normalises `.` and `..` by their components,
refuses a path that is not a folder, and bumps the lookup generation. The
process's working directory is never changed, so the golden harness's
per-case folder and the interface's file root hold whatever the code does.
Since cycle U4 `cd` is confined to the file root whenever
`Interp::file_root` is set, which every client mode does and nothing else
does: a target that is a folder whose canonical path is not the root or
inside it, compared component by component, is `Cannot CD to <dir>: it is
outside the file root.`, and one that is not a folder keeps its message;
the REPL and script mode set no root, and `cd` there goes anywhere, as
before. This keeps the command window and the file browser on one folder;
it is not a sandbox, since every file builtin still takes any absolute
path. The protocol's `cwd` moves the current folder too, through the same
`Interp::enter_folder`, to a folder judged as `files` judges one and held
in the root's plain form, so `pwd` never shows a `\\?\`, except on
Windows for a folder no plain path can spell (a name ending in a dot or a
space, or a device name, made through a `\\?\` path), which keeps its
verbatim form. `interp::normalize`, which resolves `cd`'s `.` and `..`,
builds what pushing the components one at a time gives, byte for byte,
without that walk's pushes, so it takes time linear in the path's length
on a verbatim path too, where each push rebuilt the whole path (cycle
U4).

**N-D arrays (cycle 14, in place).** The dimensions past the second sit
beside `rows` and `cols`, normalised, so a 2-D value is unchanged and every
2-D kernel stays correct for one; `builtins::ND_OK` and `nd_gate` refuse an
N-D argument to every builtin not taught one, and each consumer outside the
registry handles every dimension or refuses (see "Add a value type"). A
named display writes each page, in column-major page order, under
`name(:,:,k) =` with every index past the second, as the named display of
that page alone, a 2-D value of the array's class and storage, writes it;
`Value::display` and `Value::write_display` are the only places that know
the name, and `disp` writes the same pages under `(:,:,k) =`. The
interpreter writes them a page at a time (`write_pages`, through
`Interp::emit_display` and `emit_disp`), each header in time linear in its
length, so a display holds one page's text however many pages there are.
The Design notes of
`docs/modules/14-nd-arrays.md` have the details. Cycle 14b taught the
builtins through two kernels: every reduction is `math::each_slice` over
the array seen along its dimension as `[before, n, after]`, by the `sum`
page's rule, a dimension past `ndims` handing the argument back as a
matrix's always was, and `cat` and a bracket with an N-D operand are one
function, `interp::concat`, so they cannot disagree there, while a bracket
of 2-D operands alone keeps the 2-D rule of `hcat` and `vcat`, its
empties included (`[1:0]` is 0x0, where `cat(2, 1:0)` is 1x0); see
`docs/modules/14b-nd-functions.md`. Cycle 14c taught the search, sort
and statistics functions and the flips through the same view, a matrix
along 1 or 2 read column by column or row by row as it always was, and
added `horzcat` and `vertcat` as `cat` along 2 and 1, not as the
brackets, so `horzcat(zeros(1, 0), zeros(1, 0))` is 1x0 where the bracket
is 0x0; see `docs/modules/14c-more-nd-builtins.md`.

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
unchanged. Since cycle U3 a write the interpreter makes itself, a builtin
that writes or deletes a file or the protocol's `write_file`, goes through
`Interp::file_written`, which bumps the generation, drops every lookup, so
a file just made is found and shadows a builtin at the next call, and
drops the parse of the file written and of no other: a file rewritten with
the same length inside the file system's timestamp tick had run from its
old parse, which Save then Run meets at once, and dropping every parse
made a loop that writes a log file and calls a large function fifty times
slower. Each cached parse holds a `FileId`, its canonical path and its
path normalised, taken once when the file is parsed; a write canonicalises
the path it wrote once (a file already gone, its folder's canonical path
joined to its name) and drops each parse whose canonical or normalised
path matches, comparing paths already held, never asking the file system
once per parse, and asking nothing when no parse is cached. So a file
written through a relative path, a link or a verbatim `\\?\` path and read
through another spelling is one file. A file another program changes that
way is still not seen until its stamp moves. At most 500 calls run at once (`MAX_RECURSION`), and every frame
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
| `det([1 2; 3 4])` prints `    -2`, where the spec records MATLAB's `   -2.0000`. Cycle 02 fixed the display half: a value a rounding error from an integer now prints with decimals. The value half remains: this interpreter's pivoted elimination lands exactly on `-2`, because the last product `3 * 0.66666666666666674` is a rounding tie that goes to the even `2`, so there is nothing for the display to show. MATLAB's `-2.0000` implies LAPACK returns `-2.0000000000000004`, an operation order not reproduced here. Cycle 08 replaced `det` with the shared LU and kept the old elimination order on purpose, since no source at hand settles LAPACK's; its spec forbids choosing an order for the digits it gives. Cycle 15 found no MathWorks page that states the digits | later (verify first) |
| An error text says more than MATLAB's and keeps its own wording: the dimension mismatch names the operator and both shapes, where MATLAB says only `Arrays have incompatible sizes for this operation.` | by design; see the message-text policy in `docs/modules/01e-display-and-parser.md` |
| Numerics, cycle 09: a solver that fails (`fzero` with no sign change, `fminsearch` at its cap, a divergent `integral`, `ode45` below its smallest step) is a clean error, where MATLAB warns and returns a value or `NaN`; `polyfit` with too few points warns with `\`'s rank-deficient text; `ode45` with one output gives a struct of `solver`, `x` and `y` only; several message texts are SplatCrab's own. The Design notes of `docs/modules/09-numerics.md` have each. Cycle 10 replaced the complex refusals of `roots` with the values | by design |
| Linear algebra, cycle 08: a system singular only to working precision warns with MATLAB's exactly-singular text, where the MathWorks `mldivide` page settles MATLAB's as the nearly singular warning, `Matrix is close to singular or badly scaled. Results may be inaccurate. RCOND = ...`, issued "When `rcond` is between `0` and `eps`", which needs a condition estimate SplatCrab does not compute; a rank-deficient least-squares system warns with SplatCrab's own `Matrix is rank deficient to working precision (rank r).`; `det` is exactly `0` wherever `\` warns, where MATLAB's is the product of the pivots; `eig`, `svd`, `rank`, `pinv`, `null`, `orth` and `cond` refuse a `NaN` or `Inf`, where the MathWorks `eig` page settles that `eig` "returns `NaN` values when the input contains nonfinite values", the one-output answer, and gives no shape for a two-output call, which stays verify first; `eig([])` is 0x1. The nearly singular warning and `eig` of nonfinite input are settled by their pages (cycle 15) and recorded for a later cycle. The Design notes of `docs/modules/08-linear-algebra.md` have each | later; the two-output `eig` of nonfinite input later (verify first) |
| Complex numbers, cycle 10: every builtin not on `builtins::TAKES_COMPLEX` refuses a complex argument (`sort`, `max`, `min`, `floor`, `mod`, `num2str`, `reshape`, `inv`, `det`, the solvers, since cycle 14c `flip` and `circshift`, and every other), where MATLAB takes many of them; `if`, `while`, `&`, `\|`, `~`, `&&` and `\|\|` refuse a complex value; indexing, concatenation and assignment drop an all-zero imaginary part as arithmetic does, and a zero imaginary part of either sign is read as `+0`, on a branch cut and in the display; the complex display, its scale factor and the phase of complex eigenvectors are SplatCrab's; `eig` and `roots` of complex input are refused. The Design notes of `docs/modules/10-complex.md` have each | by design (verify first) |
| Chained indexing `x(2:3)(2)` is read successively, as Octave does; MATLAB refuses it. `x()` is "Only 1-D and 2-D indexing is supported." where MATLAB returns `x`. Both recorded in cycle 03's Design notes | later |
| An `MException` is minimal: `message`, `identifier`, `stack` and `class`, with no `cause` or `Correction`, and its display is SplatCrab's one line `  MException (id): msg` rather than MATLAB's property listing. An error the interpreter raises itself has an empty identifier, where MATLAB's carry one such as `MATLAB:UndefinedFunction`. `e.stack` (cycle 07) holds the function frames only, not the script's own, and `file` is empty for a function local to the script that was run | later |
| Cells and structs, cycle 07: a cs-list is not spread into index subscripts (`x(c{:})`) nor accepted as a target list (`[c{:}] = deal(0)`); several message texts are recalled or SplatCrab's own. The Design notes of `docs/modules/07-cells-and-structs.md` have each. Cycle 15 removed three clauses the MathWorks pages settle: cells and structs transpose (the `transpose` and `ctranspose` pages), `isequal` compares them element by element (the `isequal` page), and `varargin` with no extra inputs is 0x0, as the `varargin` page says (`varargin_no_extra_inputs`) | later (verify first) |
| `exist` gives `5` for every builtin, where MATLAB gives `2` for the builtins it ships as `.m` files (`linspace`, for instance): every SplatCrab builtin is built in. What `exist` gives for a function local to the running script is not settled by the MathWorks page; SplatCrab gives `0`, and no case asserts it | by design; the local-function value later (verify first) |
| The trace names a function alone, `  in g3 (line 8)`, where MATLAB writes `Error in script>g3 (line 8)`; the spec fixes SplatCrab's form | by design |
| Functions are more permissive than MATLAB's in three ways, none of which changes what a file MATLAB accepts means: a function in a file on the path can call a local function of the script being run (invariant 4 reads the script's local functions after the running file's, where MATLAB keeps local functions private to their file); a file may mix functions that end with `end` and functions that do not; and a script's local functions may go without `end`. Calling a script with arguments or for a value is `Too many input arguments.` or `Too many output arguments.`, where MATLAB names the script | later |
| Command syntax judges "is a variable" when the source is lexed, from the workspace and the names assigned earlier in the source, so `x = 1; clear x; x -1` stays the expression; MATLAB judges a file the same way, the command line from the live workspace | by design; see cycle 04's Design notes |
| Function handles, cycle 06: `func2str` renders an anonymous function from its parse tree, so `@(x) (x)` reads back `@(x)x`, dropping parentheses the tree does not need, which the MathWorks `func2str` page's examples do not show either way; the space after the parameter list is dropped as the page's `@(x)x.^2+7` drops it (cycle 15, `func2str_spacing`); the trace names an anonymous function `  in @(n)g(n)` with no line; an anonymous call counts against the recursion limit of 500; `str2func` of a text that is not a name makes a handle that fails only when called. The Design notes of `docs/modules/06-function-handles.md` have each | by design (verify first) |
| Strings and files, cycle 11: `delete` refuses a wildcard rather than expand it; the regular-expression engine refuses backreferences, lookaround, atomic groups, possessive quantifiers, conditionals and inline flags, since it runs in linear time; `str2num` reads literals and operators only, where MATLAB hands its text to `eval`; `input` of text that is not an expression is an error, where MATLAB asks again; `feof` is set by a read that ends at the end of the file; a compressed MAT-file is refused, and the integer and `single` classes load as doubles; a struct array with no fields past 1,048,576 elements is refused by `load` and `save`, since no bytes of the file bound it; `save` in an empty workspace is an error rather than a file of a header alone, which `load` would refuse; `fopen` takes no machine format or encoding, and text is UTF-8 both ways; `strcmp` and `strcmpi` compare a char of several rows held in a cell by its whole shape, `strcmp({['ab'; 'cd']}, ['ab'; 'cd'])` being 1, which the MathWorks `strcmp` page neither refuses nor defines (it takes "a character array" of several rows and says "If used on unsupported data types, strcmp always returns 0"), verify first (cycle 15); several message texts are SplatCrab's own. The Design notes of `docs/modules/11-strings-and-io.md` have each | by design (verify first) |
| `warning('off')`, `warning('on')` and `lastwarn` do not exist: `warning('off')` prints `Warning: off` | later |
| The environment, cycle 13: `format` has `short` and `long` only; `whos` has no heading row and its layout is SplatCrab's; `ls` and `dir` print one name per line with no `.` or `..`, and `dir` returns no `date` or `datenum`; a bare `pause` waits for Enter rather than any key; `evalc` drops the final line end of what it captured, as the spec records; `datestr` takes date numbers or one date vector and no format; `exit(n)` takes 0 to 255; `which` and `help` do not report local functions. The Design notes of `docs/modules/13-environment.md` have each | by design |
| N-D arrays, cycles 14, 14b and 14c: every builtin not on `builtins::ND_OK` refuses an N-D argument with `N-D arrays are not supported by '<name>'.` (`num2str`, `mat2str`, the strings, the sets, the linear algebra, `trapz`, `cumtrapz`, `filter`, `interp1`, `cellfun` and every other not taught N-D in cycles 14b and 14c), where MATLAB takes most of them, and so does `save -ascii`; `cat`, and since cycle 14c `horzcat` and `vertcat`, take arrays only, where MATLAB's join cells too, and `flip`, `circshift`, `fliplr` and `flipud` refuse a cell or a struct, which their pages allow; `arrayfun` with `'UniformOutput', false` refuses an N-D argument, since SplatCrab's cells are never N-D; `median` answers a double whatever `A`'s class, where its page keeps `A`'s; an N-D array grows only through as many subscripts as it has dimensions or more, a linear subscript past its end and a subscript past the end of the fold fewer subscripts index being the ambiguous-growth error. The display's edges are the spec's stated rules, verify first: the per-page class line of a logical or char page, the `(:,:,k) =` headers of `disp`, and the `2×0×3 empty double array` wording of an empty N-D array; the page layout itself is the MathWorks page's. The Design notes of `docs/modules/14-nd-arrays.md`, `docs/modules/14b-nd-functions.md` and `docs/modules/14c-more-nd-builtins.md` have each. Cycle 15 took the display's edges, and growth and deletion through fewer subscripts than dimensions, to the MathWorks "Multidimensional Arrays" page, which does not settle them: its displays are of double pages alone, with no logical or char page, no `disp` of an N-D array and no empty N-D array, and it says nothing of growth or deletion through fewer subscripts, nor of a colon over an empty target taking the right-hand side's extent (cycle 14's rule, `x = []; x(:, :, :) = reshape(1:8, 2, 2, 2)` making a 2x2x2), which stays SplatCrab's stated rule, verify first | later for the rest of the builtins; the display later (verify first) |
| A `-0` run through `cumsum` along a dimension of size 1 within `ndims` is `+0`, verify first: `cumsum(-0, 1)` is `+0`, each element its own running sum from `+0`, as a matrix's always was, while past `ndims` it is handed back, so `1/cumsum(-0, 3)` is `-Inf`. The MathWorks `cumsum` page says it "returns `A` if `dim` is greater than `ndims(A)`" and nothing of a dimension of size 1, so it does not settle the case. Recorded in testing cycle 14b; cycle 15 narrowed the row to `cumsum`, since the `sum` and `mean` pages settle theirs ("or when `size(A,dim)` is `1`", now `-0`, `sum_mean_dim_of_size_one`); `prod`, `max`, `min` and `cumprod` keep a `-0` by their own arithmetic | later (verify first) |
| Hexadecimal and binary literals, cycle 16: a literal is a double, where MATLAB stores it as the smallest unsigned integer type that holds its value, or as its suffix's type (the MathWorks "Hexadecimal and Binary Values" page: "By default, MATLAB stores the number as the smallest unsigned integer type that can accommodate it"). So `class(0x2A)` is `double` where the page's is `uint8`, and `0xFFs8` the double `-1` where the page's is the `int8` `-1`; arithmetic on a literal is double arithmetic, not the integer arithmetic of MATLAB's class, and its display is a double's; and a value past 2^53 is the nearest double, `[0xFF000000001F123As64 0x1234FFFFFFFFFFFs64]` holding `-72057594035891656 81997179153022976`, the values the page's own conversion through a double gives, where MATLAB's `int64` holds `-72057594035891654 81997179153022975` exactly. SplatCrab has no integer classes. `func2str` and a handle's display show a literal by its value, `@() 0x2A` as `@()42`, since `func2str` renders from the parse tree (the cycle 06 row), and a negative one as the negation of its magnitude, `@() 0xFFs8^2` as `@()(-1)^2`. The Design notes of `docs/modules/16-hex-binary-literals.md` record it | by design, until integer classes exist |
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
Cycle 14 narrowed "Constructors take two sizes only" to `repmat` and
`cell`, and removed the Known deviations row for indexing into or growing a
second page, both by building N-D arrays. Cycle 14b narrowed the row to
`cell`, by teaching `repmat` N-D.
Cycle 15 took the verify-first rows to their MathWorks pages and removed
three: the colon operand that is not a scalar, pinned as the error it was;
the string functions that read a char matrix held in a cell as one row of
its code units, which now refuse one as they refuse any value that is not
text; and `isequal` of handles, which now compares them. It removed the
Known deviations row on `any` and `all`, narrowed the `-0` row to `cumsum`
and the rows of cycles 06 and 07 to what no page settles, and the rows no
page settles now cite their pages. It re-recorded the remainder of the
string-functions row, the builtins that read a name from a char of
several rows, which no page was taken to, as a row of its own, and
recorded a bound found in testing: a program's memory is judged one array
at a time.
Cycle 16 fixed the hex and binary literals row (QA D30): `0x1F` and
`0b101` are numbers, stored as doubles, which Known deviations records.
Cycle 17 fixed the row on the history file read whole with no byte bound:
an entry is at most 64 KiB and `history::load` reads at most the file's
last 4 MiB, compacting a longer file to what it kept.
Fixed rows are removed from the table rather than marked done, but an
instruction a removed row carried is re-recorded, never dropped with it.

A row scheduled to a roadmap module that already lists it in its Scope stays
there. A row scheduled to a bug-fix cycle (01c, 01d, 01e) is written into that
cycle's spec when the spec is drafted. When a cycle fixes a row that a later
spec also lists, it removes the row from that spec in the same commit.

| Bug | Symptom | Fixed in |
|---|---|---|
| `1:NaN` is an empty, verify first | `1:NaN` is 1x0; Octave 8.4 gives the 1x1 `NaN` and MATLAB is unverified, so cycle 01d deliberately left it as it found it while refusing the infinite end points beside it. No golden case asserts either way. Cycle 15 took it to the MathWorks `colon` page, which does not settle it: its operands are each "a real numeric scalar" and its empty results are listed, but it names no rule for a `NaN` operand | later (verify first) |
| `for` over a matrix with no rows iterates (QA D35), verify first | `for q = zeros(0, 3), disp(size(q)), end` iterates three times with `q` 0x1; Octave 8.4 iterates zero times. Do not encode either behaviour without a source that settles it. Cycle 01d left it as it found it. Cycle 15 took it to the MathWorks `for` page, which does not settle it: its "numel(valArray(1,:))" indexes a row a 0-by-n array does not have, and "a maximum of `n` times" allows fewer than `n` | later (verify first) |
| `cell` takes two sizes only | `cell(2, 3, 4)` is "N-D arrays are not supported."; MATLAB builds the 2-by-3-by-4 cell. Since cycle 01c a trailing size of `1` is dropped, as MATLAB drops it, so `cell(2, 3, 1)` is 2x3, and any other third or later size, `0` included, is that clean error. Cycle 14 built N-D arrays and taught the other constructors (`zeros`, `ones`, `rand`, `NaN`, `Inf`, `true`, `false`) and `reshape` to make them, which narrowed this row from "Constructors take two sizes only" to `repmat` and `cell`, and cycle 14b taught `repmat`, which narrowed it to `cell`. `eye` is unaffected: MATLAB rejects `eye(r, c, p)` too | later, needs N-D cells |
| Command syntax does not see an implicit `ans` in a script, verify first | Whether a name is a variable is judged when the source is lexed, from the names the script has assigned so far, and `3;` assigns `ans` only at run time. So `3;` then `ans -1` is the command `ans('-1')`, an index error, where the REPL and `--protocol`, which see the live workspace, read an expression. What MATLAB does is not settled by its documentation. Found in testing cycle 04. Cycle 15 took it to the MathWorks "Choose Command Syntax or Function Syntax" page, which does not settle it: MATLAB uses "the current workspace, and path" at the command line while the Code Analyzer and the Editor "operate without reference to the path or workspace", and the page says nothing of a name a script assigns implicitly | later (verify first) |
| Name arguments read a char of several rows as one row, verify first | `isfield(struct('ab', 1), {['a'; 'b']})` is 1, reading the column as `'ab'`, its code units in column-major order; `rmfield`, `getfield`, `setfield`, `str2func`, `feval` and `cellfun` read a field or function name the same way. Cycle 15 settled the text functions whose pages take character vectors, which now refuse such a char (the `iscellstr` page's note: "Most text-processing functions and conversion functions require input cell arrays to contain only character row vectors"); these builtins were not taken to their pages, and keep the reading the removed string-functions row recorded | later (verify first) |
| A program's memory is bounded one array at a time | Every array is judged against `MAX_ELEMS`, and every cell or struct array against `MAX_BYTES`, as it is made, but nothing bounds what a program holds in all, and storing a matrix copies it: `m = repmat('a', 2, 8e6); for k = 1:100000, C{k} = m; end` asks for 1.6 TB in 16 MB pieces, and the allocator aborts the process (exit 134). The same holds for the outputs a program asks for through a target list it builds and evaluates: `deal`, and `arrayfun` and `cellfun` (whose outputs are judged one at a time, and of an N-D array only by their lists of sizes, cycle 14c), make one copy per output, and `arrayfun` reserves a value per element before it calls anything; and a cell holding N-D values counts each element as one value, not the list of sizes it carries, so `repmat` of one can copy far more than the byte budget sees. Found in testing cycles 15 and 14c; the paths predate them | later, needs a spec (shared matrix storage or a budget for the whole program) |

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
overflow exits 134, which is distinguishable from both. Since cycle U2,
`--ui` answers inside a thread scope that waits for ever on its accepting
thread, so a panic could not unwind to that join: `server::serve` catches
it on the interpreter thread and exits 101 itself. A new one belongs in
this table, and a new golden case must assert the exit code and not only the
message text — a case that checks the message alone would pass on the very
abort it was written to catch.

One residual risk is recorded rather than fixed: if the 256 MB thread cannot
be spawned at all, `main` falls back to running on the main thread, whose 1 MB
would be exhausted well before 10,000 levels. Nothing observed has ever taken
that branch.

Three entries are marked "verify first": `for` over a matrix with no rows,
`1:NaN`, and command syntax with an implicit `ans` in a script. All three
were found by reasoning about MATLAB rather than by running it. Cycle 15
took each to its MathWorks page, the `for` page, the `colon` page and the
"Choose Command Syntax or Function Syntax" page, and none of them settles
it; nor does Octave. Confirm the real behaviour before writing a test that
asserts either way: an expected-output file that encodes a guess is worse than
no test. A fourth, the colon operand that is not a scalar, which cycle 01e
added when it taught the parser to read `1:2:3:4`, was settled by the
`colon` page as the error it already was ("`colon` now returns an error
when creating vectors if one or more operands are not scalar", R2025a):
cycle 15 pinned it (`colon_nonscalar_operand`) and removed its row.

Two former "verify first" rows are gone. The loop variable after a
zero-iteration `for` was settled as a real bug and cycle 01d fixed it. `mod`
and `rem` with an infinite divisor went the other way: Octave 8.4 and MATLAB's
own documented formulas `a - m.*floor(a./m)` and `a - b.*fix(a./b)` all give
the `NaN` this interpreter already gives, and nothing supports the `5` the row
once suspected, so cycle 01d removed the row without a change. That is still
not a real MATLAB run, so **no golden case asserts either value**: the
evidence is strong enough to stop calling it a bug, not strong enough to pin.
