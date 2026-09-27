# 03 — Indexing forms

## Goal

logical indexing read/write, deletion `x(i) = []` (row-vector rule, "only one non-colon index"), in-place `assign_index` (validate-then-mutate, no clone), shared `resolve_read/resolve_write/gather`, lexer `{ } . @`, `Expr::Access(name, Vec<Access>)` + `LValue`, `Stmt::MultiAssign` with `~`, `nargout`-aware `max min sort size find`

After this module a user can do things that were not possible before; every
bullet in Scope must be demonstrable by at least one acceptance test below.

## Scope

- logical indexing read/write
- deletion `x(i) = []` (row-vector rule, "only one non-colon index")
- in-place `assign_index` (validate-then-mutate, no clone)
- shared `resolve_read/resolve_write/gather`
- lexer `{ } . @`
- `Expr::Access(name, Vec<Access>)` + `LValue`
- `Stmt::MultiAssign` with `~`
- `nargout`-aware `max min sort size find`
- Trailing singleton subscripts (QA D22): `A(2, 1, 1)`, `A(:, :, 1)` and
  `A(1, 2, 1) = 9` work, with `end` equal to 1 in a third position, and a
  third index past 1 is the usual "Index in position 3 exceeds array bounds"
  error
- A logical mask is a mask, never a list of indices, including a mask with
  no zeros (QA D6): `x(x > 0)` on `[5 6 7]` is `5 6 7`, not `5 5 5`
- The size-overflow message for indexed growth names the size asked for:
  `x = []; x(1e300) = 1` reports `1x1e+300`, not `usize::MAX`. Keep the
  requested size as `f64` and judge it with `args::check_shape`, whose
  `fmt_dim` formatting cycle 01c added for constructors and the colon;
  `check_size`, which growth reaches today, renders the saturated `usize`

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.
- Evaluating brace, field and dynamic-field access at runtime. This cycle
  lands the access-chain AST and lexer tokens only; cycles 06 and 07 use them.

## Design notes

Filled in as decisions are made. Start from the
"Key designs to preserve" section of docs/ARCHITECTURE.md and record here:
which files and types changed, which invariants were preserved (column-major
storage, 1-based to 0-based conversion at the index boundary, the `end` stack,
name resolution order, the output sink), and any deviation from MATLAB that
was accepted deliberately.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/03-indexing-forms/`. Expected output is written by hand from MATLAB
semantics wherever feasible; the `.err` substring is always chosen by hand.
Every new error message needs an `err_*` case.

1. `x = [5 3 8 1]; disp(x(x > 2))` → `     5     3     8`
2. `x = 1:6; x(x > 4) = 0; disp(x)` → `     1     2     3     4     0     0`
3. `A = [1 2 3; 4 5 6; 7 8 9]; disp(A(A > 5)')` → `     7     8     6     9`
4. `x = 1:5; x(2) = []; disp(x); x(logical([1 0 1 0])) = []; disp(x)` → `     1     3     4     5\n     3     5`
5. `A = [1 2 3; 4 5 6]; A(:, 2) = []; disp(A); A(1, :) = []; disp(A)` → `     1     3\n     4     6\n     4     6`
6. `A = [1 2; 3 4]; A(2) = []; disp(size(A))` → `     1     3`; `A = [1 2; 3 4]; A(1, 2) = []` → err `A null assignment can have only one non-colon index.`
7. `[m, i] = max([3 9 2])` → `m =\n\n     9\n\ni =\n\n     2\n`
8. `[r, c] = size(zeros(2, 5)); fprintf('%d %d\n', r, c); [~, i] = min([4 2 8]); disp(i); [s, idx] = sort([3 1 2]); disp(idx)` → `2 5\n     2\n     2     3     1`
9. `[r, c] = find([0 1; 1 0]); disp([r c])` → `     2     1\n     1     2`
10. `A = zeros(2); A(:) = 1:4; disp(A); x = []; x(3) = 1; disp(x)` → `     1     3\n     2     4\n     0     0     1`
11. `x = 1:5; x(end+1) = 6; x(end) = []; disp(numel(x)); x(0)` → `     5` then err `Index in position 1 is invalid. Array indices must be positive integers.`
12. `[a, b] = 5` → err `Insufficient number of outputs from right hand side of equal sign to satisfy assignment.`; `[a, b] = sum([1 2])` → err `Too many output arguments.`
13. Perf guard: `z = []; for k = 1:200000, z(end+1) = k; end; disp(numel(z))` → `    200000` well under one second.
14. Parser unit tests: `x{2}`, `s.a`, `s.(n)`, `c{1}(2).b` produce the expected `Access` chains; `[a, ~, c] = f(x)` parses to `MultiAssign`.
15. `A = [1 2; 3 4]; disp(A(2, 1, 1)); disp(A(:, :, 1)); A(1, 2, 1) = 9; disp(A)` → `     3\n     1     2\n     3     4\n     1     9\n     3     4`; `A(1, 1, 2)` → err `Index in position 3 exceeds array bounds. Index must not exceed 1.`
16. `x = [5 6 7]; disp(x(x > 0)); x(x > 0) = 0; disp(x)` → `     5     6     7\n     0     0     0`
17. `x = []; x(1e300) = 1` → err containing `Requested 1x1e+300 array`, exit code 1.

## Status

Planned
