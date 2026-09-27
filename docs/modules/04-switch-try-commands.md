# 04 — Switch try commands

## Goal

`switch/case/otherwise` (numeric, char, `case {…}`), `try/catch e` with `e.message/identifier/stack` via a minimal struct, `error('id:x', fmt, …)`, `rethrow lasterr warning assert isequal`, command syntax (`hold on`, `format long`, `clear x y`), block comments `%{ %}`

After this module a user can do things that were not possible before; every
bullet in Scope must be demonstrable by at least one acceptance test below.

## Scope

- `switch/case/otherwise` (numeric, char, `case {…}`)
- `try/catch e` with `e.message/identifier/stack` via a minimal struct
- `error('id:x', fmt, …)`
- `rethrow lasterr warning assert isequal`
- command syntax (`hold on`, `format long`, `clear x y`)
- block comments `%{ %}`

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.


## Design notes

Filled in as decisions are made. Start from the
"Key designs to preserve" section of docs/ARCHITECTURE.md and record here:
which files and types changed, which invariants were preserved (column-major
storage, 1-based to 0-based conversion at the index boundary, the `end` stack,
name resolution order, the output sink), and any deviation from MATLAB that
was accepted deliberately.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/04-switch-try-commands/`. Expected output is written by hand from MATLAB
semantics wherever feasible; the `.err` substring is always chosen by hand.
Every new error message needs an `err_*` case.

1. `x = 2; switch x, case 1, disp('one'), case {2, 3}, disp('two or three'), otherwise, disp('other'), end` → `two or three`
2. `s = 'abc'; switch s, case 'xyz', disp(1), case 'abc', disp(2), end; switch 5, case 'abc', disp(3), end` → `     2`
3. `for k = 1:5, switch k, case 3, break, end, fprintf('%d', k); end; fprintf('\n')` → `12`
4. `try, error('boom'), catch e, disp(e.message), disp(isempty(e.identifier)), end` → `boom\n     1`
5. `try, error('MyPkg:myid', 'Value %d bad', 7), catch e, disp(e.identifier), disp(e.message), end` → `MyPkg:myid\nValue 7 bad`
6. `try, x = [1 2] * [3 4]; catch, disp('caught'), end; try, try, error('in'), catch e, rethrow(e), end, catch e2, disp(['outer: ' e2.message]), end` → `caught\nouter: in`
7. `try, undefined_thing + 1, catch e, disp(e.message), end` → `Undefined function or variable 'undefined_thing'.`
8. `assert(true); assert(1 == 2, 'nope %d', 3)` → err `nope 3`
9. `disp(isequal([1 2], [1 2])); disp(isequal('a', 'a', 'a')); disp(isequal([1 2], [1 2 3]))` → `     1\n     1\n     0`
10. Command syntax: `x = 1; y = 2;\nclear x\ndisp(exist('x')); disp(exist('y'))` → `     0\n     1`; `x = 3; x -1` → `ans =\n\n     2\n` (variable, so expression); `format long` accepted as a command.
11. `%{\nthis is\na block comment\n%}\ndisp(1)` → `     1`
12. `switch [1 2], case 1, end` → err `SWITCH expression must be a scalar or a character vector.`
13. `warning('careful %d', 1)` → stderr `Warning: careful 1`, exit code 0.

## Status

Planned
