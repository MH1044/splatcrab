# 13 — Environment

## Goal

raw-console line editor via `kernel32`/`termios` FFI (no crate), history file, tab completion from registry + variables + path, `help which whos format eval evalc run pwd cd ls dir datestr now clock pause getenv system version exit`

After this module a user can do things that were not possible before; every
bullet in Scope must be demonstrable by at least one acceptance test below.

## Scope

- raw-console line editor via `kernel32`/`termios` FFI (no crate)
- history file
- tab completion from registry + variables + path
- `help which whos format eval evalc run pwd cd ls dir datestr now clock pause getenv system version exit`

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.
- A full-screen editor or debugger. Those are a later concern.

## Design notes

Filled in as decisions are made. Start from the
"Key designs to preserve" section of docs/ARCHITECTURE.md and record here:
which files and types changed, which invariants were preserved (column-major
storage, 1-based to 0-based conversion at the index boundary, the `end` stack,
name resolution order, the output sink), and any deviation from MATLAB that
was accepted deliberately.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/13-environment/`. Expected output is written by hand from MATLAB
semantics wherever feasible; the `.err` substring is always chosen by hand.
Every new error message needs an `err_*` case.

1. `format long; disp(pi); format short; disp(pi)` → `   3.141592653589793\n    3.1416`
2. `help sum` → the registry help line (e.g. `SUM Sum of elements.`).
3. Helper `myf.m` beginning `% MYF does things\n% second line`; `help myf` → ` MYF does things\n second line`
4. `eval('q = 6 * 7;'); disp(q); s = evalc('disp(1)'); disp(numel(s))` → `    42\n     6`
5. `which sum` → `built-in (sum)`; `which addone` → absolute path of `addone.m`.
6. `x = 1:3; s = 'ab'; whos` → locked two-column table (`1x3   24  double`, `1x2    4  char`).
7. `disp(ischar(pwd)); disp(ischar(datestr(now)))` → `     1\n     1`
8. Unit tests for the line-editor state machine (key events → buffer/cursor/history index) and history-file round trip.
9. `t = tic; pause(0.05); disp(toc(t) >= 0.04)` → `     1`
10. REPL transcript: pipe `1+1\nx = 3;\nx\nexit\n` into the binary with no args; lock the transcript as a golden.

## Status

Planned
