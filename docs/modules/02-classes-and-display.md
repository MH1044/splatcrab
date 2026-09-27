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
  `true(n)`, `false(n)`, `char`, `double` and `isa`
- MATLAB display rules: logical and char array headers, integer column
  widths, the common scale factor `1.0e+03 *`, and typed empty headers
- Console UTF-8 on Windows via a `SetConsoleOutputCP` FFI call, so the
  multiplication sign in size headers renders
- Indexed assignment into a char keeps it char: `s = 'abc'; s(1) = 'X'` must
  give `Xbc`, not `88 98 99`. Growth likewise
- `&&` and `||` reject non-scalar and empty operands, as MATLAB does
- Wide matrices wrap into `Columns N through M` blocks instead of printing on
  one long line
- Empty-result shapes match MATLAB: `find([])` and `diag([])` are `0x0`,
  `size('')` is `0 0`, `s(:)` on a char is a column, and `disp([])` prints
  nothing at all

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

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/02-classes-and-display/`. Expected output is written by hand from MATLAB
semantics wherever feasible; the `.err` substring is always chosen by hand.
Every new error message needs an `err_*` case.

1. `x = 5 > 3` → `x =\n\n  logical\n\n   1\n`
2. `x = [1 2 3] > 1` → `x =\n\n  1×3 logical array\n\n   0   1   1\n`
3. `disp(class(5)); disp(class('a')); disp(class(true)); disp(class(1:3 > 2)); disp(class(true + true)); disp(class(~1)); disp(class(max([true false])))` → `double\nchar\nlogical\nlogical\ndouble\nlogical\nlogical`
4. `s = 'abc'; disp(s + 0); s(2) = 'Z'; disp(s); disp(class(s(2)))` → `    97    98    99\naZc\nchar`
5. `disp(['a' 66]); disp([65 'a']); disp(class([true 2])); disp(class([true false])); disp(class(['a' true]))` → `aB\nAa\ndouble\nlogical\nchar`
6. `x = true(1,3); x(2) = 5; disp(class(x)); disp(x)` → `logical\n   1   1   1`; `y = [1 2 3]; y(2) = 'a'; disp(y)` → `     1    97     3`
7. `disp(double('A')); disp(char([72 105])); disp(logical([2 0 -1]))` → `    65\nHi\n   1   0   1`
8. `logical(NaN)` → err `NaN's cannot be converted to logicals.`
9. `x = zeros(0,3)` → `x =\n\n  0×3 empty double matrix\n`; `y = 1:0` → `y =\n\n  1×0 empty double row vector\n`; `disp([])` prints nothing.
10. Display fidelity (verify against real MATLAB): `x = 1000` → `        1000`; `x = [1 1000]` → `           1        1000`; `x = 1234.5` → `   1.2345e+03`; `x = [1.5 1000.5]` → `   1.0e+03 *\n\n    0.0015    1.0005`; `x = [0 1.5]` → `         0    1.5000`; `x = [0.001 0.002]` → `   1.0e-03 *\n\n    1.0000    2.0000`; `x = -0` → `     0`; `det([1 2; 3 4])` → `   -2.0000`.
11. `for k = 'abc', fprintf('%s:%s ', class(k), k); end; fprintf('\n')` → `char:a char:b char:c `
12. `if 'abc', disp(1), end; if [], disp(2), end; if [1 0], disp(3), end` → `     1`
13. `[1 2] && 1` → err `Operands to the || and && operators must be convertible to logical scalar values.`

## Status

Planned
