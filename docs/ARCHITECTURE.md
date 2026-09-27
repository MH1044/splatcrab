# Architecture

```
 .m source ──► lexer.rs ──► parser.rs ──► interp.rs ──► value.rs
              tokens       Stmt / Expr   tree-walking   column-major
                                         evaluator      f64 matrices
```

`src/lib.rs` exposes the four modules. `src/main.rs` is the CLI and REPL and is
the only file allowed to use `print!`.

## The modules

**`lexer.rs`** turns source into `Vec<Token>`. Two MATLAB quirks live here
rather than in the parser, because they need character-level context:

- Inside `[ ]`, whitespace separates elements. `[1 -2]` is two elements and
  `[1 - 2]` is one. The lexer tracks a stack of open delimiters and inserts a
  `Comma` when whitespace sits between something that ends a value and
  something that starts one. A newline inside brackets becomes `Semi`.
- A quote is a transpose after a value and a string delimiter otherwise. The
  `ends_value` helper decides which.

**`parser.rs`** is recursive descent with MATLAB's precedence, loosest first:
`||`, `&&`, `|`, `&`, comparison, `:`, `+ -`, `* / \ .* ./`, unary `- ~`,
`^ .^`, transpose. `end` and a bare `:` are only accepted inside an index
argument list, tracked by the `in_index` counter. `Expr`, `Stmt`, `BinOp` and
`Token` derive `PartialEq` so tests can compare trees directly.

**`value.rs`** holds `Matrix` and `Value`. Matrices are **column-major**, the
same as MATLAB: element `(r, c)` lives at `data[c * rows + r]`. This is not an
implementation detail. It is what makes `A(:)`, `reshape`, and linear indexing
produce MATLAB's answers, and every new operation must respect it.

**`interp.rs`** walks the tree. It resolves `name(args)` as indexing when
`name` is a variable and as a builtin call otherwise, grows arrays on indexed
assignment, and holds the builtin library.

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
6. **Errors are values, not panics.** Every fallible path returns
   `Result<_, String>`. A panic is a bug; the REPL must survive any bad input.

## Recipes

### Add a builtin

Until cycle 01 lands the registry, builtins are match arms in `call_builtin`
in `src/interp.rs`, returning `R<Option<Value>>` where `None` means the builtin
produces no value. Add the arm, then:

1. Add a golden case exercising it, and an `err_*` case for each new error.
2. Add a row to `docs/FEATURES.md`.
3. Add the name to the builtin list in `README.md`.

Use the `mat`, `scalar` and `dim` closures already defined in `call_builtin`
for argument access; they produce the MATLAB-style error messages.

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

**Errors (cycle 01b).** `MError { msg, line }`. Every message text is defined
in one place. Script mode prints `Error: Line N: <msg>`. The `stack` field and
the `  in <fn> (line N)` trace wait for cycle 05, since a stack only means
something once user functions exist; design `MError` so adding it is additive.

**Registry (cycle 01).** `BuiltinFn = fn(&mut Interp, &[Value], usize) -> R<Vec<Value>>`,
where the `usize` is `nargout`. An empty `Vec` means the builtin produced no
value: legal at statement level, "Too many output arguments." in an expression.
Copy the function pointer out of the map before calling it, or the borrow
checker will object to `&self` and `&mut self` at once.

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

Found while writing the cycle-0 unit tests and during the adversarial pass over
the baseline, recorded rather than silently patched. Each is scheduled to a
module; none is fixed in cycle 0, which changes no interpreter behaviour.

| Bug | Symptom | Fixed in |
|---|---|---|
| Line continuation defeats the bracket whitespace rule | `[1 ...` newline `-2]` yields one element worth `-1` instead of two elements. The `...` branch in `lex` skips past the newline and leaves the cursor on the minus, so the branch that inserts the separating `Comma` never runs. Writing a leading space on the continued line is correct by accident | 01b |
| No elementwise left divide | `a.\b` is `unexpected character '.'`. There is no `DotBackslash` token and no elementwise left-division operator | 01b |
| Chained ranges are rejected | `1:2:3:4` is a parse error; MATLAB reads it as `(1:2:3):4`. `parse_range` handles at most two colons and does not loop | later, low impact |
| `matmul` swallows `Inf` and `NaN` | `[Inf 0] * [0; 1]` gives `0`; MATLAB gives `NaN`. The `if b == 0.0 { continue }` sparsity shortcut skips the multiply, so `Inf * 0` and `NaN * 0` never happen | 08 |
| `solve` uses an absolute pivot tolerance | `[1e-15 0; 0 1e-15] \ [1; 1]` reports a singular matrix, but it is diagonal and perfectly conditioned; only its scale trips the fixed `1e-14` threshold. The threshold should be relative to the matrix norm. This also means `det` and `solve` disagree about what singular means | 08 |
| `%d` saturates at 64 bits | `fprintf('%d', 1e30)` prints `9223372036854775807`. Any integral value at or above `2^63` prints the clamp. `Inf` and `NaN` are handled correctly | 11 |
| `printf` ignores precision on strings | `fprintf('[%5.2s]', 'abcdef')` gives `[abcdef]`; C and MATLAB give `[   ab]`, truncating before padding | 11 |
| **`num2str` of an infinity panics** | `num2str(Inf)` aborts the process with "attempt to add with overflow". `log10(Inf)` is `Inf`, the cast to `i32` saturates, and the `+ 5` overflows. A panic, not an error, so it kills the REPL. Violates invariant 6 | 01 |
| **A huge size argument panics** | `zeros(1e10)` aborts with "attempt to multiply with overflow" when `rows * cols` is computed. Reachable through `ones`, `rand`, `eye`, `reshape`, `repmat` and index growth. MATLAB raises a catchable error. Violates invariant 6 | 01 |
| `sort` scrambles finite values when any element is `NaN` | `sort([5 4 NaN 2 1])` gives `4 5 NaN 1 2`; MATLAB gives `1 2 4 5 NaN`. The comparator maps an incomparable pair to `Equal`, which is not a total order, so the sort misplaces the finite elements too | 01 |
| `clear` with an argument wipes the whole workspace | `clear('a')` clears everything. The argument is never read. Silent data loss | 01 |
| `cumsum` and `cumprod` ignore the dimension argument | `cumsum([1 2; 3 4], 2)` gives the dimension-1 answer. `docs/FEATURES.md` claims the dimension argument works, and the baseline case never passes one | 01 |
| Reductions accept an out-of-range dimension | `sum(A, 3)` and `sum(A, 0)` both return the dimension-2 answer. MATLAB returns `A` unchanged for a singleton dimension and errors on `0`. `size(A, 0)` returns 1 where MATLAB errors | 01 |
| `printf` ignores the `+`, space and `#` flags | `fprintf('[%+d]', 5)` gives `[5]`; C and MATLAB give `[+5]`. The flags are parsed and discarded | 01 |
| `printf` ignores precision on integer conversions | `fprintf('[%.3d]', 5)` gives `[5]`; C gives `[005]` | 01 |
| `%d` with a non-integer falls back to `%g` | `fprintf('%d', pi)` gives `3.14159`; MATLAB switches to `%e` and gives `3.141593e+00` | 01 |
| `%s` with a number prints the number | `fprintf('%s', 65)` gives `65`; MATLAB gives `A` | 01 |
| A char argument is not expanded per character | `fprintf('[%d %d]', 'AB')` gives truncated output; MATLAB expands the char array to one argument per character and gives `[65 66]` | 01 |
| `sign(NaN)` is 0 | MATLAB gives `NaN`. The NaN case falls through to the zero branch | 01 |
| Negative sizes are rejected instead of giving an empty | `zeros(-1)` errors; MATLAB treats a negative dimension as 0 and gives `0x0` | 01 |
| Constructor arguments to constants are discarded | `NaN(2)` and `Inf(2,3)` return a scalar with the arguments silently ignored; MATLAB fills a matrix. `true(n)` and `false(n)` are cycle 02 | 01 |
| Builtins never reject extra arguments | `abs(1, 2)` returns 1 and `disp('a','b')` prints `a`. Only lower bounds are checked, so a whole class of typos passes silently | 01 |
| Reductions on an empty with an explicit dimension collapse to a scalar | `sum([], 1)` gives the scalar `0`; MATLAB gives a `1x0` empty. The no-dimension forms are all correct | 01 |
| A continuation straight after a digit fails to lex | `a = 1...` newline `+ 2;` is "unexpected character '.'". The number lexer's exclusion list omits the dot itself, so `1...` lexes as `1` then a stray `..`. Distinct from the bracket-whitespace continuation bug, and in the same list as the `2.\x` trap | 01b |
| Indexed assignment into a char silently makes it numeric | `s = 'abc'; s(1) = 'X'` yields `88 98 99` rather than `Xbc`. Indexed growth and string indexing are both claimed for the baseline; the class conversion is silent | 02 |
| `&&` and `\|\|` accept non-scalar and empty operands | `[1 1] && 1` gives 1; MATLAB requires operands convertible to a logical scalar and errors. Short-circuiting itself is correct | 02 |
| Wide matrices print on one unwrapped line | `linspace(1, 2)` prints roughly 1300 characters; MATLAB wraps into `Columns 1 through 13` blocks | 02 |
| Empty-result shapes differ in several builtins | `find([])` and `diag([])` give `0x1` where MATLAB gives `0x0`; `size('')` gives `1 0` where MATLAB gives `0 0`; `s(:)` on a char gives a row where MATLAB gives a column; `disp([])` prints `[]` where MATLAB prints nothing | 02 |
| `mod` and `rem` with an infinite divisor, unverified | `mod(5, Inf)` gives `NaN`; C `fmod` semantics suggest `5`. Not checked against a real MATLAB, so confirm before acting. Every other `mod` and `rem` edge tested is correct | verify first |
| Loop variable after a zero-iteration `for`, unverified | After `for k = []; end` the variable keeps its previous value. MATLAB may assign the empty instead. Not checked against a real MATLAB | verify first |

A trap to remember when adding `.\`: the number lexer's "do not swallow the
dot" exclusion list covers `*`, `/`, `^` and the quote, but not the backslash,
so `2.\x` already lexes as matrix left division. Adding `.\` without adding the
backslash to that list would leave `2.\x` silently meaning `2 \ x`.

The rows above in bold are process-killing panics. They break invariant 6,
"errors are values, not panics", and are the first thing cycle 01 fixes.

Two entries are marked "verify first". They were found by reading the code and
reasoning about MATLAB, not by running MATLAB, and the entries
say so. Confirm the real behaviour before writing a test that asserts either
way: an expected-output file that encodes a guess is worse than no test.
