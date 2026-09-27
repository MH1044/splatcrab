# Architecture

```
 .m source ──► lexer.rs ──► parser.rs ──► interp.rs ──► value.rs
              tokens       Stmt / Expr   tree-walking   column-major
              + lines      + lines       evaluator      f64 matrices
                                              │
                                         builtins/
                                         the library, behind a registry

                            error.rs: MError, and every message text
```

`src/lib.rs` exposes the six modules. `src/main.rs` is the CLI and REPL and is
the only file allowed to use `print!`. It runs everything on a thread with a
256 MB stack, because Windows gives the main thread 1 MB and the parser and
the evaluator each recurse once per nesting level.

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

**`parser.rs`** is recursive descent with MATLAB's precedence, loosest first:
`||`, `&&`, `|`, `&`, comparison, `:`, `+ -`, `* / \ .* ./ .\`, unary `- ~`,
`^ .^`, transpose. `end` and a bare `:` are only accepted inside an index
argument list, tracked by the `in_index` counter. `Expr`, `Stmt`, `BinOp` and
`Token` derive `PartialEq` so tests can compare trees directly. A block is a
`Vec<Located>`, where `Located` is a `Stmt` plus the line it starts on; the
line sits on a wrapper so the tree shape stays comparable on its own.

**`error.rs`** holds `MError { msg, line }`, the `R<T>` alias every fallible
path returns, a `bail!` macro, and a constructor for every message the
interpreter can raise. Nothing else in the crate spells a message out; a unit
test scans the other files for an `Err(`, `bail!(` or `ok_or_else` handed a
literal or a `format!` and fails if it finds one. `MError::at` records a line
only if none is known yet, so the innermost statement wins.

**`value.rs`** holds `Matrix` and `Value`. Matrices are **column-major**, the
same as MATLAB: element `(r, c)` lives at `data[c * rows + r]`. This is not an
implementation detail. It is what makes `A(:)`, `reshape`, and linear indexing
produce MATLAB's answers, and every new operation must respect it.

**`interp.rs`** walks the tree. It resolves `name(args)` as indexing when
`name` is a variable and as a builtin call otherwise, and grows arrays on
indexed assignment. It no longer knows what any individual builtin does.

**`builtins/`** is the library: `mod.rs` holds the registry, `args.rs` the
argument helpers, and `core.rs`, `math.rs` and `linalg.rs` the builtins
themselves. Every one has the same shape,
`fn(&mut Interp, &[Value], usize) -> R<Vec<Value>>`, where the `usize` is
`nargout` and an empty `Vec` means the builtin produced no value.

## Invariants

These hold everywhere. Breaking one is a bug even if the tests pass.

1. **Column-major storage.** See above.
2. **One-based to zero-based conversion happens at exactly one boundary**,
   in `eval_index_args`. Everything downstream of it is zero-based; everything
   in user-facing error messages is one-based.
3. **`end` is resolved through `end_stack`**, pushed per index argument with
   the size of the dimension being indexed. A function call must never push
   to it, or `x(f(end))` would bind `end` to the wrong thing.
4. **Name resolution order is variable first, then builtin.** A user variable
   shadows a builtin of the same name, as in MATLAB: after `sum = 3`, `sum(1)`
   indexes the variable. Later modules insert user functions and path files
   between the two.
5. **All interpreter output goes through `Interp::emit`.** No `print!` outside
   `src/main.rs`. Tests swap `Interp.out` for a buffer to capture output.
6. **Errors are values, not panics.** Every fallible path returns `R<T>`,
   which is `Result<T, MError>`. A panic is a bug; the REPL must survive any
   bad input. Not restored yet: see "Known bugs".

## Recipes

### Add a builtin

1. Write the function in the right file under `src/builtins/`: `core.rs` for
   constants, constructors, shape queries, output, the workspace and timing;
   `math.rs` for element-wise and reducing numerics; `linalg.rs` for linear
   algebra, rearrangement, search and sort. The signature is
   `fn(&mut Interp, &[Value], usize) -> R<Vec<Value>>`; return `one_mat(m)`
   for a value and `none()` for a builtin that produces none.
2. Add one line to that file's `register`, with a one-line help string. The
   table is `#[rustfmt::skip]`ed so it stays one line per name.
3. Bump `EXPECTED` in the registry test in `src/builtins/mod.rs`.
4. Add a golden case exercising it, and an `err_*` case for each new error.
5. Add a row to `docs/FEATURES.md` and the name to `README.md`.

Use the helpers in `src/builtins/args.rs` for argument access; they produce
the MATLAB-style messages. `need` and `at_most` bound the argument count,
`mat`, `scalar` and `string` fetch one, `dim` reads a dimension argument
(a positive integer), `size_arg` reads a size (a negative size is `0`), and
`check_size` is the only sanctioned way to turn a user-supplied shape into an
allocation length.

### Add a statement

1. `lexer.rs`: add the keyword to the `match` in the identifier branch so it
   lexes as a `Token` rather than an `Ident`.
2. `parser.rs`: add the `Stmt` variant and a branch in `parse_stmt`.
3. `interp.rs`: add the arm in `exec`, returning the right `Flow`.
4. `main.rs`: if the statement opens a block, teach `needs_more` to count it so
   the REPL keeps reading lines.

### Add a value type

`Value` is currently `Mat` or `Str`. Growing it (logical and char classes in
cycle 02, cells and structs in cycle 07) follows one rule: class is a property
of the array, storage stays numeric. `Matrix` gains a `class` tag rather than
becoming generic, because MATLAB's own semantics work that way (`'a' + 1` is
`98`, `true + true` is `2`) and because every numeric kernel then keeps
compiling untouched. Only the constructors of results decide the class.

## Key designs to preserve

These are decided and should not be re-litigated inside a cycle. The full
rationale is in the plan that produced this repo.

**Errors (cycle 01b, in place).** `MError { msg, line }` in `error.rs`, with
every message text defined there and nowhere else. Script mode prints
`Error: Line N: <msg>`; the REPL prints `Error: <msg>`, since a REPL entry is
one line. A statement's line is attached in `exec_block` by `MError::at`,
which keeps the first line it is given, so an error inside a `for` body
reports the body's line. The `stack` field and the `  in <fn> (line N)` trace
wait for cycle 05, since a stack only means something once user functions
exist; adding the field is additive, because no call site formats a message
itself.

**Registry (cycle 01, in place).** `BuiltinFn = fn(&mut Interp, &[Value], usize) -> R<Vec<Value>>`,
where the `usize` is `nargout`. An empty `Vec` means the builtin produced no
value: legal at statement level, "Too many output arguments." in an expression.
Copy the function pointer out of the map before calling it, or the borrow
checker will object to `&self` and `&mut self` at once. `Stmt::Expr` asks for
0 values and `eval` asks for 1; cycle 03 adds the call sites that ask for more.

**Classes (cycle 02).** `Class { Double, Logical, Char }` as a tag on `Matrix`.
Arithmetic yields `Double`; comparisons and logical operators yield `Logical`;
concatenation yields `Char` if any operand is `Char`, else `Logical` if all
are, else `Double`. Indexed assignment keeps the left-hand side's class.

**Frames (cycle 05).** A stack of `Frame { vars, end_stack, unit, func_name }`
with `frames[0]` as the base workspace, never popped. Moving `end_stack` into
the frame is what stops `end` leaking across a call. Resolution order becomes
variable, then the running file's local functions, then the script's local
functions, then a file on the path, then a builtin. User files shadow
builtins, as in MATLAB.

**Containers (cycle 07).** `CellArray` and `StructArray` as separate types that
reuse index-resolution helpers factored out of `Matrix`, rather than making
`Matrix` generic. Writes go through a recursive `assign_chain` that creates the
right empty container when a path does not exist yet.

## Known deviations from MATLAB

Recorded as `% NOTE:` lines in the affected golden cases, and fixed in the
cycle named:

| Deviation | Fixed in |
|---|---|
| Char arrays display with quotes; MATLAB shows them bare | 02 |
| An exact zero prints as `0.0000` in a fixed-point row; MATLAB prints `0` | 02 |
| Empty values print as `[]`; MATLAB prints `1x0 empty double row vector` | 02 |
| Integer columns are too narrow for values of 1000 and above | 02 |
| No common scale factor (`1.0e+03 *`) for non-integer matrices | 02 |
| `det` of an integer matrix prints as an integer; MATLAB shows `-2.0000` | 02 |
| `who` prints a typed table; in MATLAB that is `whos`, and `who` is bare names | 13 |
| `norm` and `sort` accept vectors only | 08, 09 |
| Backslash solves square systems only, and errors instead of warning | 08 |

## Known bugs

Found while writing the cycle-0 unit tests, during the adversarial pass over
the baseline, while migrating the builtins in cycle 01 and while closing
cycle 01b; recorded rather than silently patched. Each is scheduled to a
module. Cycle 01 fixed the sixteen rows scheduled to it, including the two
process-killing panics, and cycle 01b the four scheduled to it: the two
line-continuation defects, the missing `.\`, and the uncapped range. Fixed
rows are removed from the table rather than marked done.

| Bug | Symptom | Fixed in |
|---|---|---|
| Chained ranges are rejected | `1:2:3:4` is a parse error; MATLAB reads it as `(1:2:3):4`. `parse_range` handles at most two colons and does not loop | later, low impact |
| A parse error names the token by its `Debug` name | `y = x + ;` reports `unexpected Semi in expression`; the 01b spec's own example renders it `unexpected ';' in expression`. Only the rendering is wrong: the line number and the position are right. `err_line_parse.err` therefore asserts the `Line N:` prefix alone | later, low impact |
| The REPL prints errors to stdout | `main`'s REPL branch uses `println!`, so a piped session cannot tell diagnostics from output; script mode correctly uses stderr. Visible in `repl_error_has_no_line.out`, which is why that case has no `.err` file | later, low impact |
| A size past `usize` is named as the clamp | `x = 0:1e-300:1e300` and `zeros(1e300)` both report "Requested 1x18446744073709551615 array exceeds the maximum array size.", naming `usize::MAX` rather than the size actually asked for, because `args::clamp_to_usize` saturates before `check_size` sees the value. The refusal is clean and the limit is right; only the number in the message is an artefact. Pre-existing in `check_size`; cycle 01b gave it one more way in by routing `:` through the same helper | later, low impact |
| `matmul` swallows `Inf` and `NaN` | `[Inf 0] * [0; 1]` gives `0`; MATLAB gives `NaN`. The `if b == 0.0 { continue }` sparsity shortcut skips the multiply, so `Inf * 0` and `NaN * 0` never happen | 08 |
| `solve` uses an absolute pivot tolerance | `[1e-15 0; 0 1e-15] \ [1; 1]` reports a singular matrix, but it is diagonal and perfectly conditioned; only its scale trips the fixed `1e-14` threshold. The threshold should be relative to the matrix norm. This also means `det` and `solve` disagree about what singular means | 08 |
| `%d` saturates at 64 bits | `fprintf('%d', 1e30)` prints `9223372036854775807`. Any integral value at or above `2^63` prints the clamp. `Inf` and `NaN` are handled correctly | 11 |
| `printf` ignores precision on strings | `fprintf('[%5.2s]', 'abcdef')` gives `[abcdef]`; C and MATLAB give `[   ab]`, truncating before padding | 11 |
| Indexed assignment into a char silently makes it numeric | `s = 'abc'; s(1) = 'X'` yields `88 98 99` rather than `Xbc`. Indexed growth and string indexing are both claimed for the baseline; the class conversion is silent | 02 |
| `&&` and `\|\|` accept non-scalar and empty operands | `[1 1] && 1` gives 1; MATLAB requires operands convertible to a logical scalar and errors. Short-circuiting itself is correct | 02 |
| Wide matrices print on one unwrapped line | `linspace(1, 2)` prints roughly 1300 characters; MATLAB wraps into `Columns 1 through 13` blocks | 02 |
| A non-finite element forces the whole row to four decimals | `disp([1 2 NaN])` gives `    1.0000    2.0000       NaN`; MATLAB gives `     1     2   NaN`, because a `NaN` or an `Inf` does not stop MATLAB using the integer column format. `disp(NaN)` is `       NaN` rather than `   NaN`. This is what makes the `disp` half of cycle-01 acceptance bullets 15, 23 and 25 unwritable: `sort_nan_last`, `sign_nan` and `nan_inf_constructors` assert those values through `fprintf('%g')` instead and carry a `% NOTE` saying so. When this is fixed, give those three cases back the `disp` lines the spec bullets name. Already visible in `00-baseline/display_formats` as `x6` | 02 |
| Empty-result shapes differ in several builtins | `find([])` and `diag([])` give `0x1` where MATLAB gives `0x0`; `size('')` gives `1 0` where MATLAB gives `0 0`; `s(:)` on a char gives a row where MATLAB gives a column; `disp([])` prints `[]` where MATLAB prints nothing | 02 |
| Constants take no size argument | `pi(2)`, `e(2)` and `eps(2)` are now "Too many input arguments."; MATLAB fills a 2x2. Cycle 01 gave `NaN` and `Inf` their size arguments and added arity checks everywhere, which turned the old silent discard into an error. Belongs with `true(n)` and `false(n)` | 02 |
| `sort` takes no direction | `sort(v, 'descend')` is now "Too many input arguments."; MATLAB sorts descending. The old code accepted the argument and ignored it, so it returned the ascending sort. Same family as the constants above: the cycle-01 arity checks turned a silently wrong answer into an honest error | 09 |
| `find` takes no count | `find(x, k)` is now "Too many input arguments."; MATLAB returns the first `k` nonzero indices. The old code ignored `k` and returned every index | 03 |
| `norm` takes no order | `norm(v, p)` is now "Too many input arguments."; MATLAB returns the p-norm. The old code ignored `p` and returned the 2-norm. Lands with matrix `norm` (1, 2, inf, fro) | 08 |
| `diag` takes no offset | `diag(A, k)` and `diag(v, k)` are now "Too many input arguments."; MATLAB reads the k-th diagonal, or places the vector on it. The old code ignored `k` and used the main diagonal | 08 |
| `num2str` takes no precision | `num2str(x, n)` is now "Too many input arguments."; MATLAB formats to `n` significant digits. The old code ignored `n`. Already named in cycle 11's Goal as `num2str(x,prec)` | 11 |
| `round` takes no digit count | `round(x, n)` is now "Too many input arguments."; MATLAB rounds to `n` decimal places. The old code ignored `n` and rounded to an integer, like every other element-wise unary | 09 |
| Constructors take two sizes only | `zeros(r, c, p)`, `ones(r, c, p)` and `rand(r, c, p)` are now "Too many input arguments."; MATLAB builds an r-by-c-by-p array. The old code built the 2-D array and dropped the third size. Needs N-D arrays, which no module claims yet. `eye` is unaffected: MATLAB rejects `eye(r, c, p)` too | later, needs N-D arrays |
| `max` and `min` on an empty ignore the dimension | `max([], [], 1)` gives `0x0`; MATLAB gives `1x0`. The empty check comes before the dimension argument is read. Reductions were fixed in cycle 01; `max` and `min` return an empty either way, so this one is a shape difference rather than a wrong value | 02 |
| `mod` and `rem` with an infinite divisor, unverified | `mod(5, Inf)` gives `NaN`; C `fmod` semantics suggest `5`. Not checked against a real MATLAB, so confirm before acting. Every other `mod` and `rem` edge tested is correct | verify first |
| Loop variable after a zero-iteration `for`, unverified | After `for k = []; end` the variable keeps its previous value. MATLAB may assign the empty instead. Not checked against a real MATLAB | verify first |
| `printf` checks neither width nor precision | `fprintf('%.65536f', 1)` panics with "Formatting argument out of range", because Rust's formatter holds precision in a `u16` and 65536 overflows it; 65535 is fine. `fprintf('%2147483647d', 1)` hangs instead, building a two-gigabyte pad. Both are reachable from one line of user input and kill the REPL. Pre-existing, not a cycle-01 regression | 11 |

The two process-killing panics that used to head this list, `num2str(Inf)` and
`zeros(1e10)`, were the first thing cycle 01 fixed, before a single builtin
moved. Cycle 01b closed the third, the uncapped range, by routing the `:`
operator through `args::check_size` like every builtin shape. **Two rows of
that family are still open**, both pre-existing rather than regressions:
`printf` checks neither its width nor its precision. `check_size` covers a
size that arrives at a builtin and now the `:` operator too, but not a digit
count taken from a format string. **Invariant 6 is therefore not yet
restored**, and no document should claim it is until those two rows are gone.

Because of this, `src/main.rs` joins the interpreter thread with
`unwrap_or(101)`, not `unwrap_or(1)`. A panic must stay distinguishable from a
clean error by exit code, since that is the golden harness's main tripwire: a
case with an `.err` file expects exit 1, so a panic reported as 1 could pass a
test that was meant to prove the opposite.

Two entries are marked "verify first". They were found by reading the code and
reasoning about MATLAB, not by running MATLAB, and the entries
say so. Confirm the real behaviour before writing a test that asserts either
way: an expected-output file that encodes a guess is worse than no test.
