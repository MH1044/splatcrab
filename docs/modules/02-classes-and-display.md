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

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/02-classes-and-display/`. Expected output is written by hand from MATLAB
semantics wherever feasible; the `.err` substring is always chosen by hand.
Every new error message needs an `err_*` case.

1. `x = 5 > 3` → `x =\n\n  logical\n\n   1\n`; `disp(3 > 1)` → `   1`, the
   logical width, not the double width `     1` (QA D38)
2. `x = [1 2 3] > 1` → `x =\n\n  1×3 logical array\n\n   0   1   1\n`
3. `disp(class(5)); disp(class('a')); disp(class(true)); disp(class(1:3 > 2)); disp(class(true + true)); disp(class(~1)); disp(class(max([true false])))` → `double\nchar\nlogical\nlogical\ndouble\nlogical\nlogical`
4. `s = 'abc'; disp(s + 0); s(2) = 'Z'; disp(s); disp(class(s(2)))` → `    97    98    99\naZc\nchar`
5. `disp(['a' 66]); disp([65 'a']); disp(class([true 2])); disp(class([true false])); disp(class(['a' true]))` → `aB\nAa\ndouble\nlogical\nchar`
6. `x = true(1,3); x(2) = 5; disp(class(x)); disp(x)` → `logical\n   1   1   1`; `y = [1 2 3]; y(2) = 'a'; disp(y)` → `     1    97     3`
7. `disp(double('A')); disp(char([72 105])); disp(logical([2 0 -1]))` → `    65\nHi\n   1   0   1`
8. `logical(NaN)` → err `NaN's cannot be converted to logicals.`
9. `x = zeros(0,3)` → `x =\n\n  0×3 empty double matrix\n`; `y = 1:0` → `y =\n\n  1×0 empty double row vector\n`. (`disp([])` printing nothing stood here too; cycle 01e did it, and its `empty_result_shapes` case pins it.)
10. Display fidelity (verify against real MATLAB): `x = 1000` → `        1000`; `x = [1 1000]` → `           1        1000`; `x = 1234.5` → `   1.2345e+03`; `x = [1.5 1000.5]` → `   1.0e+03 *\n\n    0.0015    1.0005`; `x = [0 1.5]` → `         0    1.5000`; `x = [0.001 0.002]` → `   1.0e-03 *\n\n    1.0000    2.0000`; `x = -0` → `     0`; `det([1 2; 3 4])` → `   -2.0000`. Also the scalar
    fixed-point range (QA D20): `x = 12345.6` → `   1.2346e+04`;
    `x = 0.001` → `   1.0000e-03`; `x = 1e10` → `   1.0000e+10`.
11. `for k = 'abc', fprintf('%s:%s ', class(k), k); end; fprintf('\n')` → `char:a char:b char:c `
12. `if 'abc', disp(1), end; if [], disp(2), end; if [1 0], disp(3), end` → `     1`
13. (Was `[1 2] && 1` → an error about a logical scalar. Cycle 01e did it, with the wording `Operands to the logical AND (&&) and OR (||) operators must be convertible to logical scalar values.`, and its `err_and_non_scalar` and `err_or_empty` cases pin it. Nothing is left here: once comparisons return `Logical`, the operands of `&&` reach the same conversion as before.)
14. `s = 'abc'` → `s =\n\n    'abc'\n`; `c = ['ab'; 'cd']` → `c =\n\n  2×2 char array\n\n    'ab'\n    'cd'\n`
15. `x = fliplr('abc'); disp(class(x)); disp(x); disp(sort('cab')); s = []; s = [s 'abc']; disp(class(s)); t = 'ab'.'; disp(size(t)); disp(class(t)); disp(class(+'a'))` → `char\ncba\nabc\nchar\n     2     1\nchar\ndouble`
16. `x = [5 6 7]; x(x > 0)` → a clean error, not `5 5 5`. Suggested text `Logical indexing is not supported yet.`; cycle 03 replaces the error with the real thing

## Status

Planned
