# Testing

Two layers. Golden files test the language as a user sees it; unit tests test
the pieces that are awkward to reach from a script.

## Commands

    cargo test                       # everything
    cargo test --lib                 # unit tests only
    cargo test --test golden         # golden cases only

    GOLDEN_FILTER=03-indexing cargo test --test golden     # one module
    UPDATE_GOLDEN=1 GOLDEN_FILTER=03-indexing cargo test --test golden

In PowerShell, set the variable first and clear it afterwards:

    $env:GOLDEN_FILTER="03-indexing"; cargo test --test golden; Remove-Item Env:GOLDEN_FILTER

## Golden cases

A case is a `.m` file with a sibling `.out`:

| File | Meaning |
|---|---|
| `<name>.m` | the script. Line 1 is `% covers: <spec bullet>` |
| `<name>.out` | exact expected stdout. May be empty |
| `<name>.err` | optional. A substring that must appear in stderr; the process must exit 1 |
| `<name>.stdin` | optional. Piped to the script's stdin |

A `.m` file is a case when it has a sibling `.out`, **or** when its first line
is the `% covers:` marker. The marker is what makes a brand new case
discoverable before its `.out` exists, so `UPDATE_GOLDEN=1` can create one; a
case with neither fails loudly rather than being skipped in silence.

A `.m` with no marker and no `.out` is a helper: a function or script file that
a case next to it calls by name. Each case runs with its own directory as the
working directory, so helpers resolve without a path.

Without an `.err` file the process must exit 0. With one it must exit 1 and
stderr must contain the substring. A case may have both an `.out` and an
`.err`: that is how "output was flushed before the error" gets tested.

Name error cases `err_*`. Group cases by module: `tests/cases/NN-name/`.

## Normalisation

Expected and actual output are both normalised before comparison:

1. CRLF becomes LF.
2. Trailing whitespace is stripped from each line.
3. Trailing blank lines are dropped.

Interior blank lines and leading whitespace are compared exactly. MATLAB's
column alignment is part of the specification, not incidental formatting.

`.gitattributes` forces LF for `.m`, `.out` and `.err`, so the Windows and
Ubuntu CI jobs compare identical bytes.

## A .out file is the specification, not a cache

`UPDATE_GOLDEN=1` rewrites `.out` files from whatever the binary currently
prints. That turns today's behaviour into "correct", including a regression.
Use it only when:

- a new case has no `.out` yet, or
- a display or behaviour change is described in the spec and the commit body.

Afterwards, read `git diff tests/cases` line by line. A diff in a module
directory other than the one being worked on is a red flag by default. Always
pair it with `GOLDEN_FILTER` so unrelated cases cannot be rewritten silently.
CI never sets `UPDATE_GOLDEN`.

`.err` files are never rewritten by the tool. The expected error text is always
chosen by hand from the spec.

## Floating point

The display format rounds to four decimals, which absorbs roundoff in most
cases. Where the last digits depend on the order of operations, make the case
self-checking instead of locking in digits:

    disp(norm(A * inv(A) - eye(3)) < 1e-10)

so the `.out` is just `1`. Use `fprintf('%.4f')` rather than letting a value
reach the display path when only a few digits matter. Never compare `%.15g`
output across platforms.

Avoid values that land exactly on a rounding tie, such as `%.1f` of `3.25`.
Different C libraries disagree, and the test then says more about the platform
than about the interpreter.

Unit tests compare floats with a tolerance, never with `==`.

## Unit tests

Each source file carries its own `#[cfg(test)] mod tests` at the bottom.

- `lexer.rs`: token streams. The whitespace rule inside brackets, quote versus
  transpose, numbers such as `2.*x` where the dot must not be swallowed, and
  identical output for CRLF and LF input.
- `parser.rs`: tree shapes and precedence. `Expr`, `Stmt`, `BinOp` and `Token`
  all derive `PartialEq`, so trees can be compared directly.
- `value.rs`: numerics with tolerances. Broadcasting, matmul shapes, `solve`
  including pivoting and singular systems, `inv`, `det`, and the display
  format branches.
- `interp.rs`: behaviour, through a capture helper that swaps `Interp.out` for
  an in-memory buffer, plus the pure helpers `fmt_e`, `fmt_g`, `format_printf`,
  `range` and `matrix_power`.

## The output rule

Interpreter output goes through `Interp::emit`, which writes to the
`Interp.out` sink. `print!` and `println!` are forbidden everywhere except
`src/main.rs`, which owns the REPL prompt, the banner and error reporting.
This is what makes output capturable in unit tests; a stray `print!` bypasses
the buffer and the test silently sees nothing.
