# 01b — Error reporting and lexer fixes

## Goal

Make an error say where it happened, and fix the three lexer defects found by
the cycle-0 unit tests and the adversarial baseline pass. Split out of cycle 01
at planning because the
registry migration and the error-type change are each wide refactors, and
doing both at once would put two ripples through the same files.

```matlab
x = 5;
y = x + ;      % Error: Line 2: unexpected ';' in expression

[1 ...
-2]            % two elements, as in MATLAB, not one worth -1

a = [4 8]; a.\2   % elementwise left divide
```

## Scope

- `src/error.rs` defining `MError { msg: String, line: Option<u32> }` and a
  `bail!`-style helper. Every error message text moves here or is constructed
  through one place, so the same message is never formatted in two spots.
- `type R<T> = Result<T, MError>` replacing `Result<T, String>`, with
  `From<String> for MError` so existing `?` sites keep compiling while they
  are converted.
- Line numbers on tokens, carried through to statements, so a runtime error
  reports the line of the statement that raised it.
- Script mode prints `Error: Line N: <msg>` on stderr and exits 1. The REPL
  prints `Error: <msg>` with no line number, since there is only one line.
- Fix the line-continuation bug: a `...` followed by a newline inside brackets
  currently skips past the newline and leaves the cursor on the next
  character, so the branch that inserts the separating `Comma` never runs.
  `[1 ...` newline `-2]` must be two elements.
- Fix the continuation-after-a-digit bug: `a = 1...` followed by a newline
  and `+ 2;` fails to lex, because the number lexer's "do not swallow the
  dot" exclusion list omits the dot itself, so `1...` becomes `1` and a
  stray `..`. This is the same exclusion list as the backslash trap below,
  so fix both in one change.
- Add elementwise left divide `.\`: a `DotBackslash` token, a `BinOp::ELDiv`,
  and the evaluation arm. Add the backslash to the number lexer's "do not
  swallow the dot" exclusion list in the same change, or `2.\x` will silently
  keep meaning `2 \ x`.

## Out of scope

- The `stack` field on `MError` and the `in <fn> (line N)` trace. A stack is
  only meaningful once user functions exist, so it belongs to cycle 05. Design
  `MError` so adding the field later is additive.
- Column numbers and source snippets in errors. Line granularity is enough.
- Chained ranges such as `1:2:3:4`, which are still rejected. Low impact; see
  "Known bugs" in `docs/ARCHITECTURE.md`.

## Design notes

**The lexer test ripple is the main cost.** 43 assertions in
`src/lexer.rs` compare against a plain `Vec<Token>`. Do not change `lex()` to
return tokens paired with lines, or every one of them has to change. Prefer
returning a struct whose `tokens` field is still a `Vec<Token>`, with a
parallel `lines: Vec<u32>` alongside, and keep a small `lex()` wrapper that
returns just the tokens for the tests. Whatever shape is chosen, say in this
section why, and keep the existing unit tests meaningful rather than merely
compiling.

**47 `R<T>` uses** across `interp.rs` and `parser.rs` go through the alias, so
the type swap is mostly transparent. The work is in the places that build an
error from a `String` literal and now want a line attached.

**Attach the line at the statement boundary**, not at every expression node.
A statement's line is enough to locate a runtime error, and it keeps the AST
small. The parser knows the line of the token that started each statement.

## Acceptance tests

Each becomes at least one golden case in `tests/cases/01b-error-reporting/`.

1. `x = 5;` newline `y = x + ;` → err containing `Line 2:`
2. A runtime error on a later line reports that line, not line 1: a three-line
   script whose third line is `[1 2] * [3 4]` → err containing `Line 3:`
3. `disp(1)` newline `disp(2)` newline `y + 1` → prints `     1` and `     2`,
   then err with `Line 3:` and `Undefined function or variable 'y'.`
4. An error inside a `for` body reports the body's line, not the loop's.
5. The REPL reports errors without a line prefix and survives them: pipe
   `bad_name`, `1+1`, `exit` and check the session continues to `ans = 2`.
6. `x = [1 ...` newline `-2]; disp(numel(x))` → `     2`, and
   `disp(x(2))` → `    -2`
7. `x = [1 ...` newline ` -2]; disp(numel(x))` → `     2`, the leading-space
   form that already worked, as a regression guard.
8. `x = [1 - 2]; disp(numel(x))` → `     1`, proving the fix did not break the
   other side of the whitespace rule.
9. `a = [4 8]; b = a.\[8 8]; disp(b)` → `     2     1`
10. `disp(2.\8)` → `     4`, and `disp(2 .\ 8)` → `     4`, proving the number
    lexer no longer swallows the dot before a backslash.
11. `disp([2 4].\[8 8])` → `     4     2`
12. `a = 1...` newline `+ 2; disp(a)` → `     3`, and the forms that already
    work stay working: `1 ...`, `x...`, `)...`, `]...` and `1.0...`.
13. Unit: `lex` produces a `DotBackslash` for `a.\b` and a `Num` followed by
    `DotBackslash` for `2.\b`, not a `Backslash`.
14. Unit: every distinct error message the interpreter can raise is
    constructed through `error.rs`, checked by grepping for stray
    `.to_string()` error literals in the evaluator.

## Status

Planned
