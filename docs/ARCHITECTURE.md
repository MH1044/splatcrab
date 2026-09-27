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

**Errors (cycle 01).** `MError { msg, line, stack }`. Every message text is
defined in one place. Script mode prints `Error: <msg>` followed by one
`  in <fn> (line N)` per stack frame.

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

Found while writing the cycle-0 unit tests, recorded rather than silently
patched. Each is scheduled to a module; none is fixed in cycle 0, which changes
no interpreter behaviour.

| Bug | Symptom | Fixed in |
|---|---|---|
| Line continuation defeats the bracket whitespace rule | `[1 ...` newline `-2]` yields one element worth `-1` instead of two elements. The `...` branch in `lex` skips past the newline and leaves the cursor on the minus, so the branch that inserts the separating `Comma` never runs. Writing a leading space on the continued line is correct by accident | 01 |
| No elementwise left divide | `a.\b` is `unexpected character '.'`. There is no `DotBackslash` token and no elementwise left-division operator | 01 |
| Chained ranges are rejected | `1:2:3:4` is a parse error; MATLAB reads it as `(1:2:3):4`. `parse_range` handles at most two colons and does not loop | later, low impact |
| `matmul` swallows `Inf` and `NaN` | `[Inf 0] * [0; 1]` gives `0`; MATLAB gives `NaN`. The `if b == 0.0 { continue }` sparsity shortcut skips the multiply, so `Inf * 0` and `NaN * 0` never happen | 08 |
| `solve` uses an absolute pivot tolerance | `[1e-15 0; 0 1e-15] \ [1; 1]` reports a singular matrix, but it is diagonal and perfectly conditioned; only its scale trips the fixed `1e-14` threshold. The threshold should be relative to the matrix norm. This also means `det` and `solve` disagree about what singular means | 08 |
| `%d` saturates at 64 bits | `fprintf('%d', 1e30)` prints `9223372036854775807`. Any integral value at or above `2^63` prints the clamp. `Inf` and `NaN` are handled correctly | 11 |
| `printf` ignores precision on strings | `fprintf('[%5.2s]', 'abcdef')` gives `[abcdef]`; C and MATLAB give `[   ab]`, truncating before padding | 11 |

A trap to remember when adding `.\`: the number lexer's "do not swallow the
dot" exclusion list covers `*`, `/`, `^` and the quote, but not the backslash,
so `2.\x` already lexes as matrix left division. Adding `.\` without adding the
backslash to that list would leave `2.\x` silently meaning `2 \ x`.
