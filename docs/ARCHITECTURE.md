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
line sits on a wrapper so the tree shape stays comparable on its own. An `if`
arm is an `IfArm`, which carries its condition's own line for the same reason:
an error in an `elseif` condition must name the `elseif`.

`Token` also has a `Display` form, which is what every parse message renders
the offending token through; its `Debug` is the Rust variant name and used to
reach the user as `unexpected Semi in expression`. The parser counts nesting
in `depth` against `MAX_DEPTH` and refuses anything deeper, both when it
recurses (`((x))`, `f(f(x))`, a nested block) and when a left-folding loop
deepens the tree without recursing (`1+1+…+1`). `Interp` counts the same way
against the same constant, so a program the parser accepts is one the
evaluator can walk; see invariant 6.

**`error.rs`** holds `MError { msg, line }`, the `R<T>` alias every fallible
path returns, a `bail!` macro, and a constructor for every message the
interpreter can raise. Nothing else in the crate spells a message out; a unit
test scans the other files for an `Err(`, `bail!(` or `ok_or_else` handed a
literal or a `format!` and fails if it finds one. `MError::at` records a line
only if none is known yet, so the innermost statement wins.

**`value.rs`** holds `Matrix`, `Class` and `Value`. Matrices are
**column-major**, the same as MATLAB: element `(r, c)` lives at
`data[c * rows + r]`. This is not an implementation detail. It is what makes
`A(:)`, `reshape`, and linear indexing produce MATLAB's answers, and every new
operation must respect it. Every `Matrix` carries a `class` tag, `Double`,
`Logical` or `Char`, over the same `f64` storage; a char element is one UTF-16
code unit. `value.rs` also owns the display: `format` for the numeric body,
`disp_text` for `disp`, and `display_body` for the class headers of a named
display.

**`interp.rs`** walks the tree. It resolves `name(args)` as indexing when
`name` is a variable and as a builtin call otherwise, and grows arrays on
indexed assignment. It no longer knows what any individual builtin does.

**`builtins/`** is the library: `mod.rs` holds the registry, `args.rs` the
argument helpers, and `core.rs`, `math.rs` and `linalg.rs` the builtins
themselves. Every one has the same shape,
`fn(&mut Interp, &[Value], usize) -> R<Vec<Value>>`, where the `usize` is
`nargout` and an empty `Vec` means the builtin produced no value.

**`main.rs`** is the CLI. On Windows it first switches the console's output
code page to UTF-8 with `SetConsoleOutputCP(65001)`, declared as a raw
`extern "system"` function under `#[cfg(windows)]`, because the crate takes no
dependencies; everything the interpreter writes is UTF-8 already.

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
   bad input. Restored in cycle 01e, which closed the last input that could
   abort the process. Holding it is what the depth limit of
   `parser::MAX_DEPTH` is for: the parser and the evaluator both recurse once
   per nesting level, and recursion bounded only by the stack cannot return a
   value when it runs out. Anything that computes a result shape from its
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
requested shape into allocation lengths. `check_size` takes sizes that are
already `usize` and serves indexed growth alone; do not use it for a new
builtin.

A shape the user never spells out goes through `check_shape` too. Since cycle
01d, any operation whose result shape is computed from its operands' shapes
calls it before allocating: `Matrix::try_zip` (and so `zip`), `Matrix::matmul`,
the two-subscript branch of `index_read` and `math::reduce`. The operands can
be tiny and the result enormous — `ones(1e5, 1) + ones(1, 1e5)` asks for 1e10
elements from 2e5 — so "the operands fit, therefore the result fits" is never
true. A new operation of that kind belongs on the same list.

### Add a statement

1. `lexer.rs`: add the keyword to the `match` in the identifier branch so it
   lexes as a `Token` rather than an `Ident`.
2. `parser.rs`: add the `Stmt` variant and a branch in `parse_stmt`.
3. `interp.rs`: add the arm in `exec`, returning the right `Flow`.
4. `syntax.rs`: if the statement opens a block, teach `completeness` to count
   it, so both the REPL and the interface keep reading lines. This lived in
   `main.rs` until cycle U0 moved it out; look there, not in the binary.

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
structs (cycle 07) become new `Value` variants beside `Mat`; the code that
matches on `Value` today is written against a single variant, so adding one
makes the compiler list every site that needs a decision.

## Key designs to preserve

These are decided and should not be re-litigated inside a cycle. The full
rationale is in the module specs under `docs/modules/`.

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

**Classes (cycle 02, in place).** `Class { Double, Logical, Char }` as a tag
on `Matrix`. Arithmetic yields `Double`; comparisons and logical operators
yield `Logical`; concatenation yields `Char` if any operand is `Char`, else
`Logical` if all are, else `Double`, with a 0x0 double left out of the vote.
Indexed assignment keeps the left-hand side's class. A char element is a
UTF-16 code unit. The Design notes of `docs/modules/02-classes-and-display.md`
have the full table and every display rule.

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
| `det([1 2; 3 4])` prints `    -2`, where the spec records MATLAB's `   -2.0000`. Cycle 02 fixed the display half: a value a rounding error from an integer now prints with decimals. The value half remains: this interpreter's pivoted elimination lands exactly on `-2`, because the last product `3 * 0.66666666666666674` is a rounding tie that goes to the even `2`, so there is nothing for the display to show. MATLAB's `-2.0000` implies LAPACK returns `-2.0000000000000004`, an operation order not reproduced here | 08, which replaces `det` with a shared LU factorisation (verify first) |
| Two error texts say more than MATLAB's and keep their own wording: the dimension mismatch names the operator and both shapes, where MATLAB says only `Arrays have incompatible sizes for this operation.` | by design; see the message-text policy in `docs/modules/01e-display-and-parser.md` |
| `x(0)` ends `... must be positive integers.` where MATLAB ends `... must be positive integers or logical values.` | 03, which is when logical values become true of this interpreter |
| `who` and `whos` print the same typed table | Both produce byte-identical output. In MATLAB `who` is a bare list of names and `whos` is a table with size, bytes and class, so both deviate rather than only `who`, and neither has a bytes column | 13 |
| `norm` and `sort` accept vectors only | 08, 09 |
| Backslash solves square systems only, and errors instead of warning | 08 |
| A result that would be complex is a clean error; MATLAB returns the value | 10 |

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
also turned QA D6 from a silent wrong answer into a clean error, which the
row below records, and narrowed the `det` deviation above to its value half.
Fixed rows are removed from the table rather than marked done, but an
instruction a removed row carried is re-recorded, never dropped with it.

A row scheduled to a roadmap module that already lists it in its Scope stays
there. A row scheduled to a bug-fix cycle (01c, 01d, 01e) is written into that
cycle's spec when the spec is drafted. When a cycle fixes a row that a later
spec also lists, it removes the row from that spec in the same commit.

| Bug | Symptom | Fixed in |
|---|---|---|
| `1:NaN` is an empty, verify first | `1:NaN` is 1x0; Octave 8.4 gives the 1x1 `NaN` and MATLAB is unverified, so cycle 01d deliberately left it as it found it while refusing the infinite end points beside it. No golden case asserts either way | later (verify first) |
| `for` over a matrix with no rows iterates (QA D35), verify first | `for q = zeros(0, 3), disp(size(q)), end` iterates three times with `q` 0x1; Octave 8.4 iterates zero times. The MATLAB `for` page's "numel(valArray(1,:))" is ambiguous for a 0-row array. Do not encode either behaviour without a real MATLAB run | 01d (verify first) |
| A non-UTF-8 file is unread (was part of QA D29) | A UTF-16LE file is `Error: Line 1: unexpected character` on a replacement character; MATLAB and Octave read it. Cycle 01e skipped the leading UTF-8 byte-order mark and swapped the strict read for a lossy one, which fixed the Windows-1252 half (a `% caf<E9>` comment now runs) and brought the failure inside the `Error:` format; a UTF-16 file still decodes to replacement characters rather than to its text, because that needs encoding detection and not a lossy decode | later |
| A colon operand that is not a scalar is an error | `[1 3]:4` is `range start must be a scalar.`, and so therefore is `1:2:3:4`, which cycle 01e taught the parser to read as `(1:2:3):4`. MATLAB is understood to take the first element of a non-scalar colon operand, which would make it `1:4`; that was not verified against a real MATLAB run, so 01e fixed the parse and left the evaluation as it found it. Verify before changing it | later (verify first) |
| Logical indexing is unsupported (QA D6) | `x = [5 6 7]; x(x > 0)` and `x(x > 0) = 0` are the clean error `Logical indexing is not supported yet.`; MATLAB gives `5 6 7` and `0 0 0`. Until cycle 02 comparisons returned doubles, so a mask of all ones selected element 1 repeatedly and gave `5 5 5`, **silently and with no error**. Cycle 02 made comparisons and the predicates logical and refused a logical index at `eval_index_args`, in reading and in assignment alike, so the silent half is gone. The workaround until then is `x(find(x > 0))`, since `find` returns positions as doubles | 03 |
| Trailing singleton subscripts are rejected (QA D22) | `A = [1 2; 3 4]; A(2, 1, 1)` is "Only 1-D and 2-D indexing is supported."; MATLAB gives `3` (Octave agrees) | 03 |
| A size past `usize` is named as the clamp: indexed growth | `x = []; x(1e300) = 1` reports `Requested 1x18446744073709551615 array exceeds the maximum array size.`, because `eval_index_args` saturates the index before `check_size` sees it. Split from the constructor row, which cycle 01c fixed; cycle 03 rewrites `assign_index` | 03 |
| Any bracketed assignment target is unsupported (QA D32) | `[r, c] = size(ones(2, 3))` is "invalid assignment target", and so is the single-output spelling `[x] = size(A, 1)`: the parser rejects a bracket on the left of `=` outright, rather than rejecting more than one output. So `[s, i] = sort(v)` and `[m, i] = max(v)` cannot be reached from any syntax the parser accepts. Cycle 03 claims `Stmt::MultiAssign` | 03 |
| `%{ … %}` block comments execute (QA D7) | `%{` newline `disp(111)` newline `%}` prints `111`; MATLAB prints nothing. Cycle 04 claims block comments | 04 |
| `error` misreads its arguments (QA D9) | `error('100% sure')` reports `100ure`; with one argument MATLAB applies no format or escape processing, so the message is `100% sure` (the `error` page). `error('MyPkg:myId', 'Value %d too big', 7)` reports `MyPkg:myId`; MATLAB reports `Value 7 too big` and attaches the identifier. `error('')` followed by `disp(2)` exits 1; "If all inputs to error are empty, MATLAB does not throw an error", so `2` prints. Cycle 04 owns `error('id:x', fmt, …)` | 04 |
| Command syntax is unsupported (QA D31) | `x = 1; clear x` is a parse error, and so are `clear all`, `format long` and `disp hello`. A newcomer hits it in the first minute. Cycle 04 claims command syntax | 04 |
| `inv` and `A^-1` of a singular matrix are errors (QA D26) | `inv([1 2; 2 4])` exits 1 with "Matrix is singular to working precision."; MATLAB prints that text as a warning and returns `Inf Inf; Inf Inf`, and `inv(0)` is `Inf` (Octave the same). It needs `warning` from cycle 04, and goes with the backslash deviation | 08 |
| `printf` conversions and flags differ from MATLAB (QA D16) | (a) `%E` and `%G` print a lower-case `e`. (b) `%s` of a non-integer uses `%g`: `sprintf('%s', pi)` is `3.14159` where the MATLAB `sprintf` page's own example gives `3.141593e+00`. (c) The `#` flag is ignored: `sprintf('%#.0f', 3)` is `3`, MATLAB `3.`. (d) The `0` flag pads a non-finite value: `sprintf('%05d', -Inf)` is `-0Inf`, MATLAB and C ` -Inf`. (e) The escapes `\xN`, `\N` (octal), `\a`, `\b`, `\f` and `\v` are not processed. (f) `%x`, `%X`, `%o` and a `*` width or precision are errors; MATLAB gives `ff` for `sprintf('%x', 255)` and `    3` for `sprintf('%*d', 5, 3)`. (g) An invalid conversion or a trailing `%` is an error; MATLAB "prints all text up to the invalid operator ... and discards the rest", so `sprintf('abc%q', 1)` is `abc`. Cycle 01d's Out of scope moved this row to 11: it is cosmetic, and cycle 11 rewrites `printf` for file output anyway. 01d kept the panics and the hang, which are not cosmetic | 11 |
| `num2str` of a matrix gives one row in column-major order (QA D13) | `num2str([1 2; 3 4])` is the 1x10 `'1  3  2  4'`; MATLAB gives the 2x4 char `'1  2'` / `'3  4'`. `num2str([1 -2 300])` spaces its columns differently too. It needed a multi-row char, which cycle 02 provides | 11 |
| `fprintf` rejects a file id and cannot return a byte count (QA D25) | `fprintf(1, 'hi\n')` is "The first argument must be a format string."; MATLAB writes `hi`, and `fprintf(2, ...)` writes to stderr. `n = fprintf('hi\n')` prints `hi` then "Too many output arguments."; MATLAB sets `n = 3`. Cycle 11 owns `fprintf(fid, ...)`; a file id of 2 needs the stderr sink that cycle 04's `warning` introduces | 11 |
| `clc` writes raw terminal escapes to stdout | `clc` emits `ESC[2J ESC[H` through the normal output sink, so a script that calls it and is piped or redirected has those bytes in its captured output. MATLAB's `clc` affects the command window, not the program's output stream. Found while writing the handbook | 13 |
| `exit` and `quit` work only as bare REPL lines (QA D28) | A script ending in `exit` fails with "Undefined function or variable 'exit'." In the REPL, `exit;`, `quit;`, `exit(3)` and an `exit` inside a block are not recognised, and the final exit code is always 0. MATLAB's `exit` ends the session, and `exit(3)` exits with code 3. Cycle 13 claims `exit` | 13 |
| Constructors take two sizes only | `zeros(2, 3, 4)` is "N-D arrays are not supported."; MATLAB builds a 2-by-3-by-4 array. The same holds for `ones`, `rand`, `NaN`, `Inf`, `true`, `false`, `reshape` and `repmat`, with separate sizes or a size vector. Since cycle 01c a trailing size of `1` is dropped, as MATLAB drops it, so `zeros(2, 3, 1)` is 2x3, and any other third or later size, `0` included, is that clean error. Before 01c, `zeros`, `ones` and `rand` with three sizes were "Too many input arguments.", and before cycle 01 they built the 2-D array and dropped the third size. The row stays, because building N-D arrays needs a design that no roadmap module claims yet. `eye` is unaffected: MATLAB rejects `eye(r, c, p)` too | later, needs N-D arrays |
| Hex and binary literals are unsupported (QA D30) | `x = 0x1F` is `unexpected Ident("x1F")`; MATLAB R2019b+ and Octave give `31` | later, low impact |

**The process-killing family.** The two panics that used to head this list,
`num2str(Inf)` and `zeros(1e10)`, were the first thing cycle 01 fixed, before a
single builtin moved. Cycle 01b closed the third, the uncapped range, by
routing the `:` operator through `args::check_size` like every builtin shape
(since cycle 01c, both go through `args::check_shape`).
The QA pass found that the family was larger than the two `printf` rows that
were left. Cycle 01d closed three of the four it added. `printf` bounds its
width and its precision, so no conversion panics and none builds a pad it
cannot afford. Every result shape computed from operand shapes now goes
through `args::check_shape` before anything is allocated: broadcasting in
`Matrix::try_zip`, `matmul`, two-subscript `index_read` and `math::reduce`
(QA D1), which is the same guard that stops a `matmul` size wrapping (QA D2).

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
