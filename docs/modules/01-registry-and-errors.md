# 01 — Registry and errors

## Goal

`src/error.rs` (`MError{msg, line, stack}`), token/statement line numbers, `Error: Line N: …` in script mode, `src/builtins/{mod,args,core,math,linalg}.rs` registry `fn(&mut Interp, &[Value], nargout) -> R<Vec<Value>>` with all 78 builtins migrated, arity checks ("Too many input/output arguments."), interpreter on a 256 MB thread, `tic`/`toc`

After this module a user can do things that were not possible before; every
bullet in Scope must be demonstrable by at least one acceptance test below.

## Scope

- `src/error.rs` (`MError{msg, line, stack}`)
- token/statement line numbers
- `Error: Line N: …` in script mode
- A builtin registry in `src/builtins/` split into `mod`, `args`, `core`,
  `math` and `linalg`, with the signature
  `fn(&mut Interp, &[Value], nargout) -> R<Vec<Value>>` and all 78 existing
  builtins migrated unchanged
- Arity checks: "Too many input arguments." and "Too many output arguments."
- The interpreter running on a 256 MB thread
- `tic` and `toc`
- Fix the line-continuation bug in the lexer: a `...` followed by a newline
  inside brackets must still leave the whitespace separator intact, so
  `[1 ...` newline `-2]` is two elements, not one worth `-1`. This cycle
  already touches the lexer to attach line numbers, so it is the natural
  place. See "Known bugs" in `docs/ARCHITECTURE.md`
- Elementwise left divide `.\`, with a `DotBackslash` token and the matching
  `BinOp`. Add the backslash to the number lexer's "do not swallow the dot"
  exclusion list at the same time, or `2.\x` will silently keep meaning
  `2 \ x`

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.
- Behaviour changes to any builtin. This cycle moves the existing 78
  builtins into the registry unchanged; the golden cases must not move.

## Design notes

Filled in as decisions are made. Start from the
"Key designs to preserve" section of docs/ARCHITECTURE.md and record here:
which files and types changed, which invariants were preserved (column-major
storage, 1-based to 0-based conversion at the index boundary, the `end` stack,
name resolution order, the output sink), and any deviation from MATLAB that
was accepted deliberately.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/01-registry-and-errors/`. Expected output is written by hand from MATLAB
semantics wherever feasible; the `.err` substring is always chosen by hand.
Every new error message needs an `err_*` case.

1. `examples/demo.m` → its current output (regression; already `00-baseline/demo_smoke`).
2. `x = 5; y = x + ` → err `Line 1: unexpected Newline in expression`.
3. `foo(1)` → err `Undefined function or variable 'foo'.`
4. `disp(1)\ndisp(2)\nx = [1 2] * [3 4];` → `     1\n     2` then err `Incorrect dimensions for matrix multiplication (1x2 * 1x2). Use '.*' for element-wise multiplication.` (stdout flushed before the error).
5. `x = disp(3)` → `     3` then err `Too many output arguments.`
6. `sum(1, 2, 3)` → err `Too many input arguments.`
7. `t = tic; x = toc(t); disp(x >= 0)` → `     1`.
8. `fprintf('%d\n', mod(-7, 3)); fprintf('%d\n', rem(-7, 3))` → `2\n-1`.
9. Unit: `fmt_g(0.0001234, 5) == "0.0001234"`, `fmt_g(123456.0, 5) == "1.2346e+05"`, `range(0.0, 0.1, 0.3).numel() == 4`, `lex("x'*y'")` gives two `Transpose`, `lex("[1 -2]")` inserts `Comma` and `lex("[1 - 2]")` does not.
10. Deep recursion of the parser/evaluator on a 256 MB thread does not crash the process (e.g. a 5000-deep parenthesised expression).

## Status

Planned
