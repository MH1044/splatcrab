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
- Cap the allocation in `range`, so `x = 1:1e15` raises a clean error instead
  of aborting in the allocator. Cycle 01 routed every builtin size through
  `args::check_size`, but `:` is an operator and never reaches a builtin, so
  this is the last member of the panic family that this cycle can close. Reuse
  `check_size`'s limit and message wording rather than inventing a second
  policy.

## Out of scope

- The `stack` field on `MError` and the `in <fn> (line N)` trace. A stack is
  only meaningful once user functions exist, so it belongs to cycle 05. Design
  `MError` so adding the field later is additive.
- Column numbers and source snippets in errors. Line granularity is enough.
- Chained ranges such as `1:2:3:4`, which are still rejected. Low impact; see
  "Known bugs" in `docs/ARCHITECTURE.md`.
- The other two open members of the panic family, `printf` width and
  precision. `fprintf('%.65536f', 1)` panics and `fprintf('%2147483647d', 1)`
  hangs. Both belong with the printf rework in cycle 11. Do not fix them here,
  and do not let any document claim invariant 6 is restored while they stand.

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

### Decisions taken while building it

**The lexer returns `Lexed { tokens: Vec<Token>, lines: Vec<u32> }`**, with
`lex(src) -> R<Vec<Token>>` kept as a one-line wrapper over
`scan(src) -> R<Lexed>`. A parallel vector rather than a `Vec<(Token, u32)>`,
for the reason the note above gives: the 43 token-stream assertions still
compare against a plain `Vec<Token>` and not one of them changed. `Token`
itself is untouched, so it stays comparable on its own — which matters for
`crlf_lexes_the_same_as_lf`, where two inputs must produce equal token
streams from different byte offsets. The lines are asserted separately, by
five new tests that read `scan(...).lines` directly, so the information is
genuinely tested rather than merely carried. Pushing to the two vectors goes
through one private `Out::push`, so they cannot drift.

**The line sits on a `Located { stmt, line }` wrapper**, not as a field on each
`Stmt` variant. A block body is `Vec<Located>`, so `Stmt` keeps comparing over
shape alone and a `u32` does not have to be threaded through every tuple
variant, including `Break` and `Continue`. The parser's statement tests now
build their expectations through an `at(line, stmt)` helper and assert the
line as well as the tree: `nested_blocks`, for instance, pins the `for` to
line 1, the `if` to line 2 and the `break` to line 3. That is a strengthening,
not a rewrite — every tree shape those tests asserted is still asserted.
`Parser::new(tokens)` still exists with an empty line table and reports line 1,
which is what the expression tests and the REPL's `needs_more` want.

**`MError::at` records a line only if none is known yet.** `exec_block` calls
it on everything a statement returns, so the innermost block that sees the
error wins and acceptance test 4 falls out for free: an error in a `for` body
reports the body's line while the `for` unwinds around it. An error in the
loop's own range expression still reports the `for`'s line, because that is the
statement that raised it.

**`Display for MError` supplies the `Line N: ` prefix**, so script mode is
still `eprintln!("Error: {}", e)` and the prefix is spelled in exactly one
place. The REPL prints `e.msg`, since a REPL entry is one line and a number
there would be noise.

**Every message text moved to `error.rs`, including the builtins'.** The spec
says "or is constructed through one place", and `args.rs` already was one for
argument messages, but leaving two homes would have left the duplication the
bullet is aimed at: `Dimensions of arrays being concatenated are not
consistent.` was formatted in two spots in `interp.rs` and
`Not enough input arguments for '{}'.` in three in `args.rs`. Each is now one
function. Acceptance test 15 is a unit test in `error.rs` that `include_str!`s
the other eight source files, cuts each at its `#[cfg(test)]`, and fails if an
`Err(`, `bail!(`, `ok_or(`, `ok_or_else(||` or `map_err(|e|` is handed a string
literal or a `format!`. That is a tighter net than grepping for `.to_string()`,
which has legitimate uses on the same lines; it was checked against three
planted violations, one of each shape, and caught all three.

**`bail!(e)` is `return Err(e.into())`.** It takes a constructor rather than a
format string on purpose, which is what stops it becoming a second place to
write messages.

**A `...` continuation is a gap between tokens exactly as whitespace is.** Both
now run the same loop and reach the same bracket-separator check, which is the
smallest fix for the reported defect and makes `[1 ...` newline `2]` two
elements as well. Adding `.` to the number lexer's exclusion list fixes
`a = 1...` in the same line of code as the backslash that `.\` needed.

**The range cap counts in `f64` before it casts.** `((b - a) / s).floor()` cast
straight to `usize` saturates, and the `+ 1` would then overflow in a debug
build, so the count is completed as a float and only then clamped. `1:1e15`
reports `Requested 1x1000000000000000 array exceeds the maximum array size.`,
`check_size`'s own wording, and exits 1 rather than aborting.

**Acceptance test 5 needed a new kind of golden case.** The runner always
spawned the binary with the case path as `argv[1]`, and `main` enters the REPL
only when there is no argument, so a piped session could not be expressed: the
`.stdin` file was delivered to a process that never reads stdin. `tests/golden.rs`
now also collects `<name>.repl` files, spawns those with no argument and pipes
the file (minus its `% covers:` marker) to the prompt. Everything else about a
case — discovery by marker, `.out`, `.err`, the exit-code rule — is unchanged,
and the 60 older cases are untouched by the change. The one `.repl` case pins
the whole session, banner and prompts included, which is what makes
`>> Error: Undefined function or variable 'bad_name'.` followed by `ans = 2` a
single assertion. It was verified against a planted regression: printing the
REPL error as `e` rather than `e.msg` makes it fail with
`+ >> Error: Line 1: Undefined function or variable 'bad_name'.`.

**Pretty token names in parse errors are not part of this cycle.** The example
at the top of this spec renders the parse error as `unexpected ';' in
expression`; the parser actually says `unexpected Semi in expression`, the
`Debug` name of the token. The Scope never promised the rendering, so
`err_line_parse.err` asserts the `Line 3:` prefix alone rather than pinning a
message the spec did not commit to. The gap between the example and the
message is recorded in "Known bugs" instead of being closed here.

**Invariant 6 is still not restored.** The `:` operator no longer aborts the
allocator, but `fprintf('%.65536f', 1)` still panics and
`fprintf('%2147483647d', 1)` still hangs. Those two rows stay open in
"Known bugs" until cycle 11.

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
13. `x = 1:1e15` -> a clean error naming the requested size, exit code 1, and
    no abort. Check the exit code is 1 and not 101: a panic now exits 101, so
    a case that only asserted the error text could pass on a panic.
14. Unit: `lex` produces a `DotBackslash` for `a.\b` and a `Num` followed by
    `DotBackslash` for `2.\b`, not a `Backslash`.
15. Unit: every distinct error message the interpreter can raise is
    constructed through `error.rs`, checked by grepping for stray
    `.to_string()` error literals in the evaluator.

## Status

Done (2026-09-27). All fifteen acceptance tests are covered: twelve script
cases and one `.repl` session under `tests/cases/01b-error-reporting/`, and
unit tests in `src/lexer.rs` and `src/error.rs` for tests 14 and 15.
Invariant 6 remains open on the two `printf` rows, which cycle 11 owns.
