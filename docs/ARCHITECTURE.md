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
`mat`, `scalar` and `string` fetch one, and `option` returns a char option
such as `'descend'` for the caller to match. `dim` reads a dimension argument
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

`Value` is currently `Mat` or `Str`. Growing it (logical and char classes in
cycle 02, cells and structs in cycle 07) follows one rule: class is a property
of the array, storage stays numeric. `Matrix` gains a `class` tag rather than
becoming generic, because MATLAB's own semantics work that way (`'a' + 1` is
`98`, `true + true` is `2`) and because every numeric kernel then keeps
compiling untouched. Only the constructors of results decide the class.

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
| An exact zero prints as `0.0000` in a fixed-point row; MATLAB prints `0` | 02 |
| Empty values print as `[]`; MATLAB prints `1x0 empty double row vector` | 02 |
| Integer columns are too narrow for values of 1000 and above | 02 |
| No common scale factor (`1.0e+03 *`) for non-integer matrices | 02 |
| `det` of an integer matrix prints as an integer; MATLAB shows `-2.0000` | 02 |
| `who` prints a typed table; in MATLAB that is `whos`, and `who` is bare names | 13 |
| `norm` and `sort` accept vectors only | 08, 09 |
| Backslash solves square systems only, and errors instead of warning | 08 |
| A result that would be complex is a clean error; MATLAB returns the value | 10 |

A row that read "Char arrays display with quotes; MATLAB shows them bare" was
removed, because it misstated MATLAB. Since R2018a, `s = 'abc'` displays as
`s =`, a blank line and `    'abc'`, with the quotes, which is what SplatCrab
already prints; `disp('abc')` is bare in both. A multi-row char shows a
`2×3 char array` header with each row quoted. Cycle 02 must keep the quotes.

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
rows where it found it. Fixed rows are removed from the table rather than
marked done.

A row scheduled to a roadmap module that already lists it in its Scope stays
there. A row scheduled to a bug-fix cycle (01c, 01d, 01e) is written into that
cycle's spec when the spec is drafted. When a cycle fixes a row that a later
spec also lists, it removes the row from that spec in the same commit.

| Bug | Symptom | Fixed in |
|---|---|---|
| An infinite range step gives an empty (was part of QA D15) | `size(1:Inf:5)` is `1 0`; MATLAB gives `1 1`, since the documented count `fix((k-j)/i)` is `0`. `range` returns early on a non-finite step, which cycle 01d left alone: its D15 bullet covered the end points only, and the two that it fixed, the colon's own end point and `linspace`'s, are gone from this table | 01e |
| `1:NaN` is an empty, verify first | `1:NaN` is 1x0; Octave 8.4 gives the 1x1 `NaN` and MATLAB is unverified, so cycle 01d deliberately left it as it found it while refusing the infinite end points beside it. No golden case asserts either way | 01e (verify first) |
| `for` over a matrix with no rows iterates (QA D35), verify first | `for q = zeros(0, 3), disp(size(q)), end` iterates three times with `q` 0x1; Octave 8.4 iterates zero times. The MATLAB `for` page's "numel(valArray(1,:))" is ambiguous for a 0-row array. Do not encode either behaviour without a real MATLAB run | 01d (verify first) |
| Chained ranges are rejected | `1:2:3:4` is a parse error; MATLAB reads it as `(1:2:3):4`. `parse_range` handles at most two colons and does not loop | 01e |
| A parse error names the token by its `Debug` name | `y = x + ;` reports `unexpected Semi in expression`; the 01b spec's own example renders it `unexpected ';' in expression`. Likewise `Ident("x1F")`, `Num(0.3)`, `RParen` and `Eof`. Only the rendering is wrong: the line number and the position are right. `err_line_parse.err` therefore asserts the `Line N:` prefix alone | 01e |
| The REPL prints errors to stdout | `main`'s REPL branch uses `println!`, so a piped session cannot tell diagnostics from output; script mode correctly uses stderr. Visible in `repl_error_has_no_line.out`, which is why that case has no `.err` file. Update that case deliberately and say so in the commit body | 01e |
| Indexed assignment into a char silently makes it numeric | `s = 'abc'; s(1) = 'X'` yields `88 98 99` rather than `Xbc`, and growth `s(4) = 'd'` yields `97 98 0 100`. Indexed growth and string indexing are both claimed for the baseline; the class conversion is silent. Also listed in cycle 02's Scope, which 01e removes it from | 01e |
| `&&` and `\|\|` accept non-scalar and empty operands | `[1 1] && 1` gives 1 and `[] \|\| 1` gives 1; MATLAB requires operands convertible to a logical scalar and errors. Short-circuiting itself is correct. Also listed in cycle 02's Scope | 01e |
| `NaN` converts silently to a logical (QA D5) | `if NaN, disp('true'), end` prints `true`; `NaN & 1` is `1` and `~NaN` is `0`. MATLAB: "NaN's cannot be converted to logicals." Octave errors too. The same conversion as the `&&` row above | 01e |
| Wide matrices print on one unwrapped line | `linspace(1, 2)` prints roughly 1300 characters; MATLAB wraps into `Columns 1 through 13` blocks. Also listed in cycle 02's Scope | 01e |
| A non-finite element forces the whole row to four decimals | `disp([1 2 NaN])` gives `    1.0000    2.0000       NaN`; MATLAB gives `     1     2   NaN`, because a `NaN` or an `Inf` does not stop MATLAB using the integer column format. `disp(NaN)` is `       NaN` rather than `   NaN`. This is what makes the `disp` half of cycle-01 acceptance bullets 15, 23 and 25 unwritable: `sort_nan_last`, `sign_nan` and `nan_inf_constructors` assert those values through `fprintf('%g')` instead and carry a `% NOTE` saying so. When this is fixed, give those three cases back the `disp` lines the spec bullets name. Already visible in `00-baseline/display_formats` as `x6` | 01e |
| Empty-result shapes differ in several builtins | `find([])` and `diag([])` give `0x1` where MATLAB gives `0x0`; `size('')` gives `1 0` where MATLAB gives `0 0`; `s(:)` on a char gives a row where MATLAB gives a column; `disp([])` prints `[]` where MATLAB prints nothing; `num2str([])` is 1x0. Also listed in cycle 02's Scope | 01e |
| Deep nesting overflows the stack (QA D4) | About 96,000 nested parentheses (`x = ((…1…));`) abort with a stack overflow, exit 134. So do `[[…]]`, `abs(abs(…))`, `x(x(…))`, and a flat `1+1+…+1` of about 280,000 terms. 5,000 levels and 100,000 flat terms pass. MATLAB gives a clean error. A depth limit in the parser, with its own message, turns this into exit 1 | 01e |
| `break` or `continue` outside a loop ends the script silently (QA D8) | `disp(1)` newline `break` newline `disp(2)` prints `1` and exits 0, and `disp(2)` never runs. MATLAB errors; Octave 8.4 gives the parse error "break must appear within a loop", and the same for `continue` | 01e |
| An error in an `elseif` condition names the `if` line (QA D27) | `if 0` newline `elseif undefined_d` newline `end` reports `Line 1`; MATLAB and Octave report line 2 | 01e |
| A UTF-8 BOM is rejected, and a non-UTF-8 file fails outside the `Error:` format (QA D29) | A file of the bytes `EF BB BF` then `disp(1)` gives `Error: Line 1: unexpected character '\u{feff}'`; MATLAB and Octave print `1`. A Windows-1252 comment (`% caf<E9>`) or a UTF-16LE file gives `Cannot read <path>: stream did not contain valid UTF-8`, which MATLAB and Octave read. A Windows editor or PowerShell produces both. The BOM fix is two lines; a lossy fallback for invalid UTF-8 is a decision for the spec | 01e |
| Several error texts differ from current MATLAB (QA D33) | `Undefined function or variable 'x'.` is `Unrecognized function or variable 'x'.` in MATLAB R2020a+; `Arrays have incompatible sizes for operator '+' (1x3 vs 1x2).` is `Arrays have incompatible sizes for this operation.`; the `x(0)` text ends `... must be positive integers or logical values.` in MATLAB. From knowledge, and secondary to behaviour. Decide the policy once and record it | 01e |
| The REPL discards an incomplete block at EOF (QA D36) | `printf 'for k=1:3\ndisp(k)\n' \| splatcrab` prints nothing and exits 0. MATLAB has no EOF equivalent; Octave reports a parse error | 01e |
| Char arrays lose their class in rearrangement (QA D17) | `fliplr('abc')` displays `99 98 97`; MATLAB gives `'cba'`. The same holds for `'ab'.'`, `flipud`, `repmat`, `reshape`, `sort('cab')` and `s = []; s = [s 'abc']`. The reverse too: `x = +'a'` stays `'a'`, where MATLAB gives `97`. Several of these need a multi-row char, which only exists once `Value::Str` is gone | 02 |
| Scalar display ignores MATLAB's fixed-point range (QA D20) | `x = 1234.5` displays ` 1234.5000`; MATLAB `   1.2345e+03`. `x = 12345.6` gives `12345.6000` flush left, `x = 0.001` gives `    0.0010` (MATLAB and Octave `1.0000e-03`), and `x = 1e10` gives `   10000000000` (MATLAB `   1.0000e+10`). Cycle 02's acceptance test 10 claims it | 02 |
| `disp` of a comparison uses the double width (QA D38) | `disp(3 > 1)` prints `     1`; MATLAB's logical display prints `   1`. README's quick tour shows it | 02 |
| A logical mask with no zeros indexes element 1 (QA D6) | `x = [5 6 7]; x(x > 0)` gives `5 5 5`, and `x(x > 0) = 0` gives `0 6 7`; MATLAB gives `5 6 7` and `0 0 0`. Comparisons return doubles today, so the mask is read as a list of indices. Cycle 02 makes comparisons logical and must then refuse a logical index rather than read it as indices; cycle 03 implements logical indexing | 03 |
| Trailing singleton subscripts are rejected (QA D22) | `A = [1 2; 3 4]; A(2, 1, 1)` is "Only 1-D and 2-D indexing is supported."; MATLAB gives `3` (Octave agrees) | 03 |
| A size past `usize` is named as the clamp: indexed growth | `x = []; x(1e300) = 1` reports `Requested 1x18446744073709551615 array exceeds the maximum array size.`, because `eval_index_args` saturates the index before `check_size` sees it. Split from the constructor row, which cycle 01c fixed; cycle 03 rewrites `assign_index` | 03 |
| Multiple assignment is unsupported (QA D32) | `[r, c] = size(ones(2, 3))` is "invalid assignment target". Cycle 03 claims `Stmt::MultiAssign` | 03 |
| `%{ … %}` block comments execute (QA D7) | `%{` newline `disp(111)` newline `%}` prints `111`; MATLAB prints nothing. Cycle 04 claims block comments | 04 |
| `error` misreads its arguments (QA D9) | `error('100% sure')` reports `100ure`; with one argument MATLAB applies no format or escape processing, so the message is `100% sure` (the `error` page). `error('MyPkg:myId', 'Value %d too big', 7)` reports `MyPkg:myId`; MATLAB reports `Value 7 too big` and attaches the identifier. `error('')` followed by `disp(2)` exits 1; "If all inputs to error are empty, MATLAB does not throw an error", so `2` prints. Cycle 04 owns `error('id:x', fmt, …)` | 04 |
| Command syntax is unsupported (QA D31) | `x = 1; clear x` is a parse error, and so are `clear all`, `format long` and `disp hello`. A newcomer hits it in the first minute. Cycle 04 claims command syntax | 04 |
| `inv` and `A^-1` of a singular matrix are errors (QA D26) | `inv([1 2; 2 4])` exits 1 with "Matrix is singular to working precision."; MATLAB prints that text as a warning and returns `Inf Inf; Inf Inf`, and `inv(0)` is `Inf` (Octave the same). It needs `warning` from cycle 04, and goes with the backslash deviation | 08 |
| `printf` conversions and flags differ from MATLAB (QA D16) | (a) `%E` and `%G` print a lower-case `e`. (b) `%s` of a non-integer uses `%g`: `sprintf('%s', pi)` is `3.14159` where the MATLAB `sprintf` page's own example gives `3.141593e+00`. (c) The `#` flag is ignored: `sprintf('%#.0f', 3)` is `3`, MATLAB `3.`. (d) The `0` flag pads a non-finite value: `sprintf('%05d', -Inf)` is `-0Inf`, MATLAB and C ` -Inf`. (e) The escapes `\xN`, `\N` (octal), `\a`, `\b`, `\f` and `\v` are not processed. (f) `%x`, `%X`, `%o` and a `*` width or precision are errors; MATLAB gives `ff` for `sprintf('%x', 255)` and `    3` for `sprintf('%*d', 5, 3)`. (g) An invalid conversion or a trailing `%` is an error; MATLAB "prints all text up to the invalid operator ... and discards the rest", so `sprintf('abc%q', 1)` is `abc`. Cycle 01d's Out of scope moved this row to 11: it is cosmetic, and cycle 11 rewrites `printf` for file output anyway. 01d kept the panics and the hang, which are not cosmetic | 11 |
| `num2str` of a matrix gives one row in column-major order (QA D13) | `num2str([1 2; 3 4])` is the 1x10 `'1  3  2  4'`; MATLAB gives the 2x4 char `'1  2'` / `'3  4'`. `num2str([1 -2 300])` spaces its columns differently too. It needs the multi-row char of cycle 02 | 11 |
| `fprintf` rejects a file id and cannot return a byte count (QA D25) | `fprintf(1, 'hi\n')` is "The first argument must be a format string."; MATLAB writes `hi`, and `fprintf(2, ...)` writes to stderr. `n = fprintf('hi\n')` prints `hi` then "Too many output arguments."; MATLAB sets `n = 3`. Cycle 11 owns `fprintf(fid, ...)`; a file id of 2 needs the stderr sink that cycle 04's `warning` introduces | 11 |
| Non-BMP characters count as one element (QA D37) | `length('😀')` is `1`; MATLAB, whose char is UTF-16, gives `2` | 11 |
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

**One member is left: deep nesting overflows the stack (QA D4), and it belongs
to cycle 01e. Invariant 6 is therefore still not restored**, and no document
should claim it is until that row is gone.

Because of this, `src/main.rs` joins the interpreter thread with
`unwrap_or(101)`, not `unwrap_or(1)`. A panic must stay distinguishable from a
clean error by exit code, since that is the golden harness's main tripwire: a
case with an `.err` file expects exit 1, so a panic reported as 1 could pass a
test that was meant to prove the opposite. An allocator abort or a stack
overflow exits 134, which is distinguishable from both.

Two entries are marked "verify first": `for` over a matrix with no rows, and
`1:NaN`. Both were found by reasoning about MATLAB rather than by running it,
and neither the MATLAB pages nor Octave settles them. Confirm the real
behaviour before writing a test that asserts either way: an expected-output
file that encodes a guess is worse than no test.

Two former "verify first" rows are gone. The loop variable after a
zero-iteration `for` was settled as a real bug and cycle 01d fixed it. `mod`
and `rem` with an infinite divisor went the other way: Octave 8.4 and MATLAB's
own documented formulas `a - m.*floor(a./m)` and `a - b.*fix(a./b)` all give
the `NaN` this interpreter already gives, and nothing supports the `5` the row
once suspected, so cycle 01d removed the row without a change. That is still
not a real MATLAB run, so **no golden case asserts either value**: the
evidence is strong enough to stop calling it a bug, not strong enough to pin.
