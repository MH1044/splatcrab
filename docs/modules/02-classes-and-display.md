# 02 — Classes and display

## Goal

`Class {Double, Logical, Char}` tag on `Matrix`, `Value::Str` removed (char matrices), class-propagation table, `class islogical ischar isnumeric logical true(n) false(n) char double isa`, MATLAB display rules (logical/char headers, integer widths, scale factor `1.0e+03 *`, empties), console UTF-8 via `SetConsoleOutputCP` FFI

After this module a user can do things that were not possible before; every
bullet in Scope must be demonstrable by at least one acceptance test below.

## Scope

- A `Class` tag on `Matrix` with the variants `Double`, `Logical` and
  `Char`, rather than making `Matrix` generic over its element type
- `Value::Str` removed; strings become char matrices
- The class-propagation table: arithmetic yields `Double`, comparisons and
  logical operators yield `Logical`, concatenation yields `Char` if any
  operand is `Char` and otherwise `Logical` only if all are, and indexed
  assignment keeps the left-hand side's class
- The builtins `class`, `islogical`, `ischar`, `isnumeric`, `logical`,
  `char`, `double` and `isa`. `true` and `false`, including the size forms
  `true(n)`, `true(r, c)` and `true(sz)` that cycle 01c added as doubles, now
  return `Logical`
- A 1-row char keeps its quotes in `x = 'abc'` display, as MATLAB has done
  since R2018a: `x =`, a blank line, then `    'abc'`. A multi-row char shows
  the `2×3 char array` header with each row quoted. A Known-deviations row
  once claimed MATLAB shows char bare; it was wrong and has been removed
- Rearrangement keeps the class (QA D17): `fliplr`, `flipud`, transpose,
  `repmat`, `reshape` and `sort` of a char return a char, and
  `[[] 'abc']` is a char. Unary plus of a char returns a double: `+'a'` is `97`
- An index of class `Logical` is a clean error until cycle 03 implements
  logical indexing (QA D6). Once comparisons return `Logical`, a mask such as
  `x > 0` must never be read as a list of indices, which is what makes
  `x(x > 0)` give `5 5 5` today
- MATLAB display rules: logical and char array headers, integer column
  widths, the common scale factor `1.0e+03 *`, and typed empty headers
- Console UTF-8 on Windows via a `SetConsoleOutputCP` FFI call, so the
  multiplication sign in size headers renders
- Indexed assignment into a char keeps it char: `s = 'abc'; s(1) = 'X'` must
  give `Xbc`, not `88 98 99`. Growth likewise. Its Known bugs row was marked
  01e and moved here by that cycle, whose Out of scope explains why: the fix
  is the `Class` tag this cycle exists to add, and doing it sooner would have
  meant inventing a temporary mechanism and then deleting it
- `s(:)` of a char is a char column. Cycle 01e gave it MATLAB's *shape*, but a
  `Value::Str` has nowhere to record one, so it comes back as character codes;
  carrying the class through it is part of QA D17 above

- The predicate builtins return `Logical`, as MATLAB's do: `any`, `all`,
  `isnan`, `isinf`, `isfinite`, `isempty`, `isscalar` and `isvector`, and the
  new `islogical`, `ischar`, `isnumeric` and `isa`. Without this, cycle 03's
  `x(isnan(x))` would read a double of ones and zeros as a list of positions.
  Added at the planning of this cycle: the propagation table above covered
  operators but not the builtins that produce masks
- A char element is a UTF-16 code unit, as in MATLAB (QA D37): `length('😀')`
  is `2`, and output decodes the units back to UTF-8. The Design notes below
  asked for this decision to be made here; storing code units makes the fix
  fall out rather than be a rework in cycle 11, so this cycle claims the row
  and removes its bullet from cycle 11's Scope in the same commit
- The three cycle-01 cases that spell with `fprintf` what their bullets spell
  with `disp` (`sort_nan_last`, `sign_nan`, `nan_inf_constructors`) get their
  `disp` lines back and lose their `% NOTE:` blocks. Cycle 01e's display fix
  made the `disp` lines correct; this cycle owns the re-blessing, per the Known
  bugs table. Covered in place, in `tests/cases/01-registry-and-builtins/`,
  not by a case in this cycle's directory
- `% NOTE:` lines in other modules' cases that describe a deviation this
  cycle fixes are removed or corrected in the same commit. One of them,
  `00-baseline/strings.m`, repeats the false claim that MATLAB shows char bare

Three bullets that stood here were fixed by cycle 01e and removed from this
Scope in the same commit: wide matrices printing on one unwrapped line, `&&`
and `||` accepting non-scalar and empty operands, and the empty-result shapes
of `find([])`, `diag([])`, `size('')` and `disp([])`. What is left of that
last one for this cycle is the class half rather than the shape half — the
typed empty headers named in the display bullet above.

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.
- Complex numbers (cycle 10) and integer classes such as int8/uint8.
- Cell and struct display, which arrives with those containers in cycle 07.

## Design notes

Filled in as decisions are made. Start from the
"Key designs to preserve" section of docs/ARCHITECTURE.md and record here:
which files and types changed, which invariants were preserved (column-major
storage, 1-based to 0-based conversion at the index boundary, the `end` stack,
name resolution order, the output sink), and any deviation from MATLAB that
was accepted deliberately.

A note from the planning of cycle 01c, for whoever designs char storage
here. MATLAB's char is a UTF-16 code unit, so `length('😀')` is `2` (QA D37,
scheduled to cycle 11). Storing Unicode scalar values makes that fix a
rework later; storing UTF-16 code units makes it fall out. Decide here, and
record the choice.

### What changed

- `src/value.rs`: `Class { Double, Logical, Char }` and a `class` field on
  `Matrix`, over the same `f64` storage. `Matrix::new` and everything built on
  it (`scalar`, `filled`, `row`, `col`, `map`, `try_map`, `zip`, `matmul`,
  `solve`) make a `Double`, so every numeric kernel is untouched and a result
  is classed only where it is constructed. `with_class` retags; `to_class`
  converts (a logical tests each element against zero and refuses `NaN`; a
  char takes each element's `code_unit`). `char_row`, `text`, `row_text` and
  `decode_units` convert between Rust strings and code units. `Value` keeps
  one variant, `Mat`: `Value::Str` is gone, and `Value::str`, `is_char`,
  `text` and `mat` replace what matching on it did. The display moved here in
  full: `format` (the numeric body), `disp_text` (what `disp` prints) and
  `display_body` (what follows `x =`).
- `src/parser.rs`: unary plus is `Expr::Pos` rather than being dropped, since
  `+'a'` must be the double `97`.
- `src/interp.rs`: a string literal evaluates to `Value::str`. `binary` tags
  comparisons, `&` and `|` `Logical` and everything else `Double`; `~`, `&&`
  and `||` give logicals. `concat_class` decides a concatenation.
  `index_read` keeps the source's class. `assign_index` converts the
  right-hand side to the target's class with `to_class`, and growth keeps it.
  `for` hands out columns in the loop value's class. `eval_index_args`
  refuses an index of class `Logical`.
- `src/builtins/`: `one_mat` now makes its result a `Double` whatever the
  argument's class; `one_as` returns a class the builtin chose. The
  rearrangements (`transpose`, `fliplr`, `flipud`, `repmat`, `reshape`,
  `sort`) use `one_as` and keep the class; the predicates and `true`/`false`
  build logicals; `max`/`min` retag their result `Logical` when every array
  argument is logical. Eight builtins are new, in `core.rs`. `args::option`
  now returns an owned `String`, and `args::string`, `dim`, `size_arg` and
  `size_list` test `is_char()` where they matched `Value::Str`.
- `src/error.rs`: `logical_indexing_unsupported`.
- `src/main.rs`: `SetConsoleOutputCP(65001)` under `#[cfg(windows)]`, a raw
  `unsafe extern "system"` declaration linked from `kernel32`, called once at
  the top of `run`. Its result is ignored: with no console (a pipe, a file,
  CI) it fails, and nothing needs it to succeed.

### Invariants

Column-major storage is untouched: the tag rides beside `data` and no layout
changed. The one-based to zero-based conversion is still made once, in
`eval_index_args`, which is also where the logical-index refusal sits, so
reading and assigning refuse alike and cycle 03 has one place to replace. The
`end` stack is pushed and popped exactly as before. Name resolution is still
variable then builtin. All output still leaves through `Interp::emit`: `disp`
and the named display build strings in `value.rs` and hand them over, and
`main.rs`'s console call changes how a console reads the bytes, not what is
written.

### The UTF-16 decision

A char element is one UTF-16 code unit, stored as an `f64` from 0 to 65535.
A literal is encoded with `str::encode_utf16` when it is evaluated, so
`length('😀')` is `2` and `double('😀')` is `55357 56832`. Everything that
turns a char back into text (`disp`, the named display, `%s`, the format
string itself, `args::string` for `clear`, `isa`, options such as
`'descend'`) decodes with `String::from_utf16_lossy`, so a surrogate pair
prints as its one character and a lone surrogate, such as `s(1)` of an emoji,
prints as U+FFFD. The source file is still read as UTF-8, so `é` is one unit.
`%c` of a single unit that is half a pair prints `?`, as it did.

A number becoming a char (`char(x)`, `['a' x]`, `s(k) = x`) is rounded to the
nearest integer and clamped to 0 to 65535, with `NaN` as 0. MATLAB's own
rule for a fraction or an out-of-range value is not recorded here, so no case
asserts it.

### Class rules chosen where the Scope is silent

- **A 0x0 double takes no part in a concatenation's class vote**, as the
  Scope requires for `[[] 'abc']`. It applies to the logical rule as well, so
  `[[] true]` is logical. When every operand is a 0x0 double the vote falls
  back to all of them, which keeps `[[] []]` a double, and a concatenation
  that is empty keeps its class, so `['' '']` is a 0x0 char.
- **A new variable, or the 0x0 double `[]`, takes the class of what is
  assigned into it**, so `s = []; s(1) = 'a'` builds a char and `n(3) = 'c'`
  creates one. Any other target keeps its class, as the Scope says.
- **Growth pads with zeros in the target's class**: a char grows with the code
  unit 0, a logical with false.
- **A logical target refuses `NaN`**: `x = true(1,2); x(1) = NaN` is
  `NaN's cannot be converted to logicals.`, the conversion `logical(NaN)`
  makes. The Scope's "a logical target stores logical(value)" implies it.
- **`max` and `min` stay logical only when every array argument is**: the
  one-array forms of a logical, and the two-array form of two logicals.
  `max(true, 2)` is a double.
- **`any` and `all` along a dimension past the array's** return the argument
  converted to logical, since that path hands back the argument itself.
- **`isa`** accepts the class names and the groups `'numeric'` and `'float'`,
  both of which hold `double` alone; `'integer'` holds nothing. The name is
  matched exactly. No other group is recognised.
- **`logical`, `char` and `double` convert any class.** `char(true)` is
  `char(1)` and `logical('a')` is true. MATLAB is understood to refuse both,
  but that is not recorded here, so this cycle converts rather than invent two
  error messages without a source; no case asserts either.
- **Only the listed rearrangements keep the class.** `diag('abc')`, the range
  `'a':'c'` and `cumsum` of a char are doubles. MATLAB's `diag` and colon
  keep a char; the Scope names neither, so they are left for a later cycle.
- **An index of class char is its codes**, as in MATLAB: `x('a')` is `x(97)`.

### Display rules

The rules below reproduce every value the Acceptance tests record. Where the
tests are silent, each is the simplest rule consistent with them; none has
been run against a real MATLAB.

- **Logical**: `0` or `1` in four-wide columns, whatever the size. The named
  display is headed `  logical` for a scalar and `  R×C logical array`
  otherwise, then a blank line. Wrapping uses the same `TERM_WIDTH` of 80, so
  twenty columns fit a block.
- **Char**: the named display of a 1-row char is `    'text'`; any other row
  count is headed `  R×C char array` and a blank line, then each row quoted
  on its own line. A char is never wrapped into column blocks. `disp` prints
  each row bare. `disp` of a char with no rows prints one empty line, as
  `disp('')` did.
- **Empties**: `  R×C empty char array` and `  R×C empty logical array` for
  every empty char and logical; for a double, `     []` when 0x0,
  `  1×C empty double row vector`, `  R×1 empty double column vector`, and
  otherwise `  R×C empty double matrix`. `1×0` takes the row-vector wording
  and `0×1` the column-vector one, as `1:0` does in the tests. `disp` of any
  empty that is not a char prints nothing. The `×` is U+00D7.
- **Integers**: when every finite element is whole and the largest magnitude
  `M` is below `1e9`. Below 1000 the column is the widest number plus three,
  at least six, as before. From 1000 it is twelve (strictly
  `max(12, digits + 2)`), which reproduces `x = 1000` and `x = [1 1000]`. A
  sign counts as a digit. The cut-over at `1e9` is chosen: `x = 1e10` must be
  `1.0000e+10`, and nine digits are the most a twelve-wide column holds with
  a sign and a margin.
- **Fixed point**: when `floor(log10(M))` is between -2 and 2, so `M` is in
  `[0.01, 1000)`: `%.4f` in ten-wide columns. An exact zero, `-0` included,
  prints as a bare `0`, right-aligned; a value that only rounds to zero keeps
  `0.0000`.
- **Outside that range**, with `k = floor(log10(M))`: a scalar is short `e`
  format, `%.4e` with a two-digit exponent in a thirteen-wide field
  (`   1.2345e+03`); an array is `A / 10^k` in the fixed-point layout, after a
  `   1.0e+0k *` line (`{:+03}` exponent) and a blank line, printed once above
  all column blocks. This single rule gives both recorded boundaries: `0.001`
  is `k = -3`, so the scalar is `1.0000e-03` and `[0.001 0.002]` is scaled by
  `1.0e-03`. It also means `[1e5 0.5]` is `1.0000 0.0000` under `1.0e+05 *`
  rather than the per-element `e` format it used to be, and that a scalar in
  `[0.001, 0.01)` such as `0.005` is now `5.0000e-03`. `10^k` is parsed from
  `1e{k}` so it is correctly rounded, and `1000.5 / 1e3` prints `1.0005`.
- **Non-finite** values take no part in choosing a layout or a width, as
  since 01e, and print as `NaN`, `Inf` and `-Inf` in every layout. A matrix of
  nothing but non-finite values is six-wide integer columns.

### Deviations accepted

- **`det([1 2; 3 4])` displays `    -2`, not the recorded `   -2.0000`.** The
  display rule is right: a value a rounding error from `-2` prints as
  `-2.0000`, and a unit test pins that. The value is the problem. This
  `det` pivots to `[3 4; 1 2]`, eliminates to `u22 = 2 - 4 * fl(1/3)`, which is
  exactly `3002399751580331 / 2^52`, and multiplies `-3 * u22`, whose exact
  result `-(2 + 2^-52)` is the midpoint between `-2` and the next double down;
  IEEE rounds the tie to the even `-2`. The other natural orders
  (`(1 * 4) / 3`, the product taken last to first, a mantissa-and-exponent
  accumulation) meet the same tie. MATLAB's recorded `-2.0000` implies its
  LAPACK returns `-2.0000000000000004` by an operation order not reproduced
  here. Cycle 08 replaces `det` with a shared LU factorisation, so the row
  moves there, marked verify first. The golden case `display_near_integer`
  pins the display half with `-2 - eps(2)`, the neighbouring double, and does
  not call `det`; `00-baseline/matrix_ops` still records the `-2` and says why.
- **The Acceptance values of item 10 are the spec's, not a MATLAB run's**, as
  the spec says, and every display rule above that no item pins is a choice,
  not a verified fact. In particular unverified: the integer width for values
  from 10,000 up, the `1e9` cut-over, the lower fixed-point bound between
  `0.001` and `0.01`, and the scale-factor layout when an array mixes very
  different magnitudes.
- **`who` and `whos` still print the same table**, now with the right class
  and a char's real shape. That deviation is cycle 13's.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/02-classes-and-display/`. Expected output is written by hand from MATLAB
semantics wherever feasible; the `.err` substring is always chosen by hand.
Every new error message needs an `err_*` case.

1. `x = 5 > 3` → `x =\n\n  logical\n\n   1\n`; `disp(3 > 1)` → `   1`, the
   logical width, not the double width `     1` (QA D38) Cases: logical_scalar_display.
2. `x = [1 2 3] > 1` → `x =\n\n  1×3 logical array\n\n   0   1   1\n` Cases: logical_row_display.
3. `disp(class(5)); disp(class('a')); disp(class(true)); disp(class(1:3 > 2)); disp(class(true + true)); disp(class(~1)); disp(class(max([true false])))` → `double\nchar\nlogical\nlogical\ndouble\nlogical\nlogical` Cases: class_propagation.
4. `s = 'abc'; disp(s + 0); s(2) = 'Z'; disp(s); disp(class(s(2)))` → `    97    98    99\naZc\nchar` Cases: char_arith_and_assign.
5. `disp(['a' 66]); disp([65 'a']); disp(class([true 2])); disp(class([true false])); disp(class(['a' true]))` → `aB\nAa\ndouble\nlogical\nchar` Cases: concat_class.
6. `x = true(1,3); x(2) = 5; disp(class(x)); disp(x)` → `logical\n   1   1   1`; `y = [1 2 3]; y(2) = 'a'; disp(y)` → `     1    97     3` Cases: indexed_assign_keeps_class.
7. `disp(double('A')); disp(char([72 105])); disp(logical([2 0 -1]))` → `    65\nHi\n   1   0   1` Cases: conversion_builtins.
8. `logical(NaN)` → err `NaN's cannot be converted to logicals.` Cases: err_logical_nan.
9. `x = zeros(0,3)` → `x =\n\n  0×3 empty double matrix\n`; `y = 1:0` → `y =\n\n  1×0 empty double row vector\n`. (`disp([])` printing nothing stood here too; cycle 01e did it, and its `empty_result_shapes` case pins it.) Cases: typed_empty_display.
10. Display fidelity (verify against real MATLAB): `x = 1000` → `        1000`; `x = [1 1000]` → `           1        1000`; `x = 1234.5` → `   1.2345e+03`; `x = [1.5 1000.5]` → `   1.0e+03 *\n\n    0.0015    1.0005`; `x = [0 1.5]` → `         0    1.5000`; `x = [0.001 0.002]` → `   1.0e-03 *\n\n    1.0000    2.0000`; `x = -0` → `     0`; `det([1 2; 3 4])` → `   -2.0000`. Also the scalar
    fixed-point range (QA D20): `x = 12345.6` → `   1.2346e+04`;
    `x = 0.001` → `   1.0000e-03`; `x = 1e10` → `   1.0000e+10`. Cases: display_integer_widths, display_scale_factor, display_scalar_range, display_near_integer.
11. `for k = 'abc', fprintf('%s:%s ', class(k), k); end; fprintf('\n')` → `char:a char:b char:c ` Cases: for_over_char.
12. `if 'abc', disp(1), end; if [], disp(2), end; if [1 0], disp(3), end` → `     1` Cases: if_condition_classes.
13. (Was `[1 2] && 1` → an error about a logical scalar. Cycle 01e did it, with the wording `Operands to the logical AND (&&) and OR (||) operators must be convertible to logical scalar values.`, and its `err_and_non_scalar` and `err_or_empty` cases pin it. Nothing is left here: once comparisons return `Logical`, the operands of `&&` reach the same conversion as before.)
14. `s = 'abc'` → `s =\n\n    'abc'\n`; `c = ['ab'; 'cd']` → `c =\n\n  2×2 char array\n\n    'ab'\n    'cd'\n` Cases: char_display.
15. `x = fliplr('abc'); disp(class(x)); disp(x); disp(sort('cab')); s = []; s = [s 'abc']; disp(class(s)); t = 'ab'.'; disp(size(t)); disp(class(t)); disp(class(+'a'))` → `char\ncba\nabc\nchar\n     2     1\nchar\ndouble` Cases: char_rearrangement.
16. `x = [5 6 7]; x(x > 0)` → a clean error, not `5 5 5`. Suggested text `Logical indexing is not supported yet.`; cycle 03 replaces the error with the real thing Cases: err_logical_index.
17. `disp(class(any(1))); disp(class(all(1))); disp(class(isnan(1))); disp(class(isinf(1))); disp(class(isfinite(1))); disp(class(isempty(1))); disp(class(isscalar(1))); disp(class(isvector(1))); disp(class(ischar('a')))` → `logical` nine times; and `disp(islogical(true)); disp(ischar('a')); disp(isnumeric('a')); disp(isnumeric(true)); disp(isnumeric(2)); disp(isa(2, 'double')); disp(isa(true, 'numeric')); disp(isa(2, 'numeric'))` → `   1\n   1\n   0\n   0\n   1\n   1\n   0\n   1` (the four-wide logical display of item 1) Cases: predicates_return_logical, class_predicates.
18. `disp(length('😀')); fprintf('%d %d\n', double('😀')); disp('😀'); disp(length('é'))` → `     2\n55357 56832\n😀\n     1` Cases: char_utf16_units.
19. `x = ''` → `x =\n\n  0×0 empty char array\n`. The wording is MATLAB's, confirmed by the titles of MathWorks Answers threads. An empty logical such as `true(0, 3)` follows the same pattern, `0×3 empty logical array`, but that text is unconfirmed and gets no golden assertion (verify first) Cases: empty_char_display.

20. Added at review, for two Scope bullets that items 4 and 15 cover only in part: `s = 'abc'; t = s(:); disp(class(t)); disp(size(t)); disp(t); u = 'ab'; u(3) = 'c'; disp(class(u)); disp(u)` → `char
     3     1
a
b
c
char
abc`, with `u(end+1)`, `flipud`, `repmat`, `reshape`, `'` and the `false`, `true(n)`, `true(sz)` and `false(r, c)` forms checked through `class` the same way. It adds coverage of existing Scope; the Scope did not change. Cases: char_column_and_growth.

Every expected output above either was recorded in this spec before cycle 02
began or is quoted from MathWorks text. A value that neither source settles is
marked verify first and gets no golden assertion.

## Status

Done (2026-09-28)
