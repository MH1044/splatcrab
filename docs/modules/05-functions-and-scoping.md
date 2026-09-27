# 05 — Functions and scoping

## Goal

`function` blocks in scripts and `.m` function files on a path, `Frame` stack (`end_stack` moves into the frame), resolution variable → local fn → script fn → path file → builtin, `nargin/nargout`, `return` (`Flow::Return`), recursion limit 500, subfunctions, scripts on the path run in the caller's workspace, file cache with generation counter, `exist feval addpath rmpath`

After this module a user can do things that were not possible before; every
bullet in Scope must be demonstrable by at least one acceptance test below.

## Scope

- `function` blocks in scripts and `.m` function files on a path
- `Frame` stack (`end_stack` moves into the frame)
- resolution variable → local fn → script fn → path file → builtin
- `nargin/nargout`
- `return` (`Flow::Return`)
- recursion limit 500
- subfunctions
- scripts on the path run in the caller's workspace
- file cache with generation counter
- `exist feval addpath rmpath`

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.
- `global` and `persistent` unless they fall out cheaply once frames exist.
- Class definitions and packages.

## Design notes

Filled in as decisions are made. Start from the
"Key designs to preserve" section of docs/ARCHITECTURE.md and record here:
which files and types changed, which invariants were preserved (column-major
storage, 1-based to 0-based conversion at the index boundary, the `end` stack,
name resolution order, the output sink), and any deviation from MATLAB that
was accepted deliberately.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/05-functions-and-scoping/`. Expected output is written by hand from MATLAB
semantics wherever feasible; the `.err` substring is always chosen by hand.
Every new error message needs an `err_*` case.

1. `disp(sq(4))\nfunction y = sq(x)\n    y = x^2;\nend` → `    16`
2. `[s, p] = sp(2, 3)\nfunction [s, p] = sp(a, b)\ns = a + b; p = a * b;\nend` → `s =\n\n     5\n\np =\n\n     6\n`
3. `disp(f(1)); disp(f(1, 2))\nfunction r = f(a, b)\nif nargin < 2, b = 10; end\nr = a + b;\nend` → `    11\n     3`
4. `disp(h()); h\nfunction r = h()\nr = nargout;\nend` → `     1` then `ans =\n\n     0\n`
5. `fprintf('%d\n', fact(10))\nfunction r = fact(n)\nif n <= 1, r = 1; else, r = n * fact(n - 1); end\nend` → `3628800`
6. `x = 1; g2(); disp(x); g3()\nfunction g2()\nx = 99;\nend\nfunction g3()\ndisp(x)\nend` → `     1` then err `Undefined function or variable 'x'.` with stack line `  in g3 (line 8)`.
7. `disp(early(5)); disp(early(-5))\nfunction r = early(x)\nr = 0; if x > 0, r = 1; return; end\nr = -1;\nend` → `     1\n    -1`
8. Helper `addone.m` beside `path_test.m` containing `disp(addone(41))` → `    42`
9. Helper `helper.m` with subfunction `twice`; `disp(helper(3)); twice(3)` → `     6` then err `Undefined function or variable 'twice'.`
10. Helper script `setup.m` (`a = 7;`); `setup; disp(a)` → `     7`
11. `sq(1, 2)` → err `Too many input arguments.` (no stack line); `z = bad(1)\nfunction y = bad(x)\nend` → err `Output argument "y" (and maybe others) not assigned during call to "bad".`
12. `inf_rec(1)\nfunction r = inf_rec(n)\nr = inf_rec(n + 1);\nend` → err `Maximum recursion limit of 500 reached.` without crashing.
13. `sq(3)` at statement level → `ans =\n\n     9\n`; `disp(feval('sq', 3))` → `     9`; `disp(exist('sq')); disp(exist('nosuch'))` → `     2\n     0`
14. Helper `shadow/max.m` returning 42; `addpath('shadow'); disp(max([1 5 2])); rmpath('shadow'); disp(max([1 5 2]))` → `    42\n     5`

## Status

Planned
