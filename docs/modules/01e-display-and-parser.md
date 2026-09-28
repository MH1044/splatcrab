# 01e — Display and parser

## Goal

Close the display and parser defects, and with them the last input that can
kill the process. After this cycle nothing a user types aborts the
interpreter, and invariant 6 can finally be claimed.

```matlab
x = ((((...1...))))       % a clean error, not a stack overflow
disp([1 2 NaN])           %      1     2   NaN, integer columns kept
linspace(1, 2)            % wraps into Columns N through M blocks
1:2:3:4                   % (1:2:3):4, as MATLAB reads it
if NaN, end               % NaN's cannot be converted to logicals
```

## Scope

Sixteen rows in the Known bugs table of `docs/ARCHITECTURE.md` are marked 01e.

**The last abort, first.**

- **Deep nesting overflows the stack** (QA D4). About 96,000 nested parentheses
  abort with exit 134, and so do nested brackets, nested calls, nested index
  reads, and a flat sum of about 280,000 terms. Give the parser an explicit
  depth limit and the evaluator the matching one, so an over-deep expression is
  a clean error rather than a crash. The 256 MB stack raised the ceiling; it
  cannot remove it, because the recursion is unbounded by nature. **This is the
  last member of the family**, and closing it is what lets `docs/ARCHITECTURE.md`
  state that invariant 6 holds.

**The parser.**

- **Chained ranges are rejected.** `1:2:3:4` is a parse error; MATLAB reads it
  as `(1:2:3):4`. `parse_range` handles at most two colons and does not loop.
- **A parse error names the token by its `Debug` name.** `y = x + ;` reports
  `unexpected Semi in expression` where it should say `';'`. The same goes for
  `Ident("x1F")`, `Num(0.3)`, `RParen` and `Eof`. Give `Token` a display form
  intended for humans, and use it in every parse message.
- **`break` or `continue` outside a loop ends the script silently** (QA D8).
  `disp(1)`, `break`, `disp(2)` prints `1` and exits 0, and the second `disp`
  never runs. MATLAB errors; Octave gives a parse error. Error.
- **An error in an `elseif` condition names the `if` line** (QA D27). Report the
  `elseif`'s own line.
- **An infinite range step gives an empty.** `size(1:Inf:5)` is `1 0`; MATLAB
  gives `1 1`, because the documented count `fix((k-j)/i)` is `0`, which is one
  element. Cycle 01d refused an infinite *end point* and deliberately left the
  *step* alone; this is that remainder.

**The REPL.**

- **Errors go to stdout.** The REPL branch of `main` uses `println!`, so a
  piped session cannot separate diagnostics from output. Script mode already
  uses stderr. This deliberately changes `repl_error_has_no_line`, whose `.out`
  currently contains the error text; move it to the `.err` and say so in the
  commit body, which the Definition of Done requires for a case outside this
  module's directory.
- **An incomplete block at EOF is discarded** (QA D36). Piping
  `for k=1:3` and `disp(k)` with no `end` prints nothing and exits 0. Report an
  unterminated block instead.
- **A UTF-8 byte-order mark is rejected** (QA D29). Three bytes `EF BB BF`
  before `disp(1)` is `unexpected character`. Skip a leading BOM. Decide and
  record what a non-UTF-8 file does, since today it fails outside the `Error:`
  format entirely.

**Display.**

- **Wide matrices print on one unwrapped line.** `linspace(1, 2)` prints about
  1300 characters; MATLAB wraps into `Columns N through M` blocks. Also listed
  in cycle 02's Scope; remove it there.
- **A non-finite element forces the whole row to four decimals.**
  `disp([1 2 NaN])` gives `1.0000 2.0000 NaN`; MATLAB keeps integer columns,
  because a `NaN` or an `Inf` is not a reason to stop using them.
- **Empty-result shapes differ in several builtins.** `find([])` and `diag([])`
  give `0x1` where MATLAB gives `0x0`; `size('')` gives `1 0` where MATLAB
  gives `0 0`; `s(:)` on a char gives a row where MATLAB gives a column;
  `disp([])` prints `[]` where MATLAB prints nothing.

**Logical conversion.**

- **`&&` and `||` accept non-scalar and empty operands.** `[1 1] && 1` gives 1
  and `[] || 1` gives 1; MATLAB requires an operand convertible to a logical
  scalar. Also listed in cycle 02's Scope; remove it there.
- **`NaN` converts silently to a logical** (QA D5). `if NaN` is taken as true,
  `NaN & 1` is 1, `~NaN` is 0. MATLAB and Octave both refuse. Same conversion
  as the row above, so do them together.

**Error text.**

- **Several messages differ from current MATLAB** (QA D33). `Undefined function
  or variable` is `Unrecognized function or variable` in R2020a and later, and
  the dimension message is worded differently. Decide a policy and record it:
  either match a named MATLAB release or state deliberately that the older
  wording is kept. Either is defensible; leaving it unstated is not. Whatever
  is chosen, apply it consistently through `error.rs` and update every affected
  `.err` file in this cycle, justified in the commit body.

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.
- **Indexed assignment into a char staying char.** The row is marked 01e, but
  the fix is the `Class` tag that cycle 02 exists to add; doing it here would
  mean inventing a temporary mechanism and then deleting it. Moved to 02, which
  already claims the char class. Record the move in the table.
- Logical indexing and the logical *class*. Cycle 02 adds the class, cycle 03
  makes masks index. This cycle only stops `NaN` and non-scalars being accepted
  where a logical scalar is required.
- N-D arrays.

## Design notes

### The depth limit

`parser::MAX_DEPTH` is **10,000 levels**, and the parser and the evaluator
share it. One number serves both because the two recursions have the same
shape: a program the parser accepts is one the evaluator can walk, with no
window in which an expression parses and then refuses to run.

*What counts as a level.* Nesting, not size. The parser deepens on entry to
`parse_expr`, which every nested expression passes through — a parenthesised
group, a matrix element, an index or call argument, the expression of a
statement — and on entry to each statement of a block, so nested blocks count
too. It also deepens **once per iteration of every left-folding loop**, which
is the half that is easy to miss: `1+1+…+1` is a left-nested tree one level
deeper per term, and the parser builds it in a loop without recursing at all.
Counting only recursion would have left a flat sum of 280,000 terms to
overflow the evaluator's stack instead of the parser's. Each loop restores the
counter when it closes, so depth measures nesting and never program length: a
2,000-statement program, or 2,000 sibling arguments, is nowhere near the
limit (`depth_is_nesting_and_not_program_length`).

*Why 10,000.* It is bounded on both sides by measurement rather than taste.
Below, by what a person or a generator might legitimately write: MATLAB code
does not nest brackets or chain operators ten thousand deep, and every golden
case and example in the repository is under twenty. Above, by the stack. The
observed abort points on the 256 MB interpreter thread were about 96,000
nested parentheses (roughly 2.7 KB per level, since one level descends the
whole twelve-function precedence ladder) and about 280,000 terms in a flat
sum (roughly 0.9 KB per level in `eval`). At 10,000 levels those are about
27 MB and about 9 MB, so **the margin is about 9x on the parser, the tighter
of the two, and about 28x on the evaluator**. A limit an order of magnitude
above real code and an order of magnitude below the stack is the widest band
available; anything much higher starts to depend on which recursion is deepest
for a given input, which is exactly the fragility the limit exists to remove.

One residual risk is recorded rather than fixed, in `docs/ARCHITECTURE.md`: if
the 256 MB thread cannot be spawned, `main` falls back to the main thread,
whose 1 MB would be exhausted long before 10,000 levels.

*What it reports.* `Nesting is too deep. The maximum nesting depth is 10000.`
from `error::nesting_too_deep`, raised identically by both, with the parser
attaching the line of the token it stopped on.

*Testing it.* The boundary tests run on a thread sized exactly as
`src/main.rs` sizes the interpreter's, because the claim being made is "the
limit fires before *that* stack does"; a test-harness thread's default is a
small fraction of it and would prove the opposite. The evaluator's half builds
its `Expr` tree directly rather than parsing one, so the parser's limit cannot
fire first and mask it. `Interp::run` resets the counters on every entry: the
REPL reuses one interpreter, and a raised counter left behind by a failed
entry would have made every later statement too deep.

### The human form of `Token`

`impl Display for Token` lives in `lexer.rs`, beside the type whose rendering
it is; `error.rs` keeps the message texts and simply formats the token with
`{}` instead of `{:?}`. One rule covers the whole form: **a token shows its
source spelling in single quotes**, so `';'`, `')'`, `'if'`, `'0.3'`, `'x1F'`.
The two tokens with no spelling say so in words, `end of line` and `end of
input`, since `unexpected 'end of input'` reads worse than `unexpected end of
input`.

Two renderings read worse than `Debug` did, and both were accepted:

- `Transpose` is a quote, so quoting it gives `'''`. Every alternative was
  worse: an unquoted `'` disappears into the surrounding prose, and a word
  ("a transpose") turns `unexpected X in expression` ungrammatical.
- `Str` and `Ident` both render as `'text'`, so `unexpected 'abc'` does not
  say which it was. The surrounding message carries that, and the alternative
  — keeping `Str("abc")` — is the defect being fixed.

### Column wrapping

`value::TERM_WIDTH` is **80 characters, always**, and deliberately not a
terminal query. The output of a script must not depend on whether it was run
at a prompt or piped into a file: the golden harness always pipes, so a wrap
that switched itself off when piped could not be tested at all, and a wrap
that followed the real terminal would make the same script print differently
on two machines. MATLAB's own command window defaults to 80 and keeps using it
when its output is captured, so the reproducible answer and the compatible one
are the same answer.

Blocks hold as many whole columns as fit: `TERM_WIDTH / width`, at least one,
so 13 integer columns of 6 or 8 fixed-point columns of 10. Each block is
headed by `  Columns N through M`, with MATLAB's two special spellings,
`  Columns N and M` for exactly two and `  Column N` for exactly one, then a
blank line, then the block's rows; blocks are separated by a blank line. A
matrix that fits carries no heading at all and prints as it always did.

### Non-finite elements in a numeric row

A `NaN` or an `Inf` no longer forces the four-decimal format, and it does not
widen the column either: the width is the widest **number** plus three, at
least six. A non-finite value has no digits, so `[NaN Inf -Inf 1]` is four
six-wide columns and `-Inf` fits in six without asking for a seventh; a matrix
with no finite element at all falls back to one digit, which is what makes
`disp(NaN)` the `   NaN` MATLAB prints.

### The error-text policy (QA D33)

**Message text is matched to MATLAB R2020a and later, except where SplatCrab's
own wording says strictly more; behaviour is matched always.** Wording is
secondary to behaviour, but a MATLAB-compatible interpreter printing a
sentence MATLAB stopped printing in 2020 is the harder position to defend, so
the default is to match.

Applied: `Undefined function or variable 'x'.` becomes **`Unrecognized
function or variable 'x'.`** through `error.rs`, and the `.err` files that
quoted the old wording change with it.

Two deliberate exceptions, both now recorded under "Known deviations from
MATLAB" in `docs/ARCHITECTURE.md` rather than as defects:

- The dimension mismatch stays `Arrays have incompatible sizes for operator
  '+' (1x3 vs 1x2).` MATLAB's `Arrays have incompatible sizes for this
  operation.` names neither the operator nor the shapes, and a message that
  says less is not an improvement. It is a deviation by design, not a debt.
- `x(0)` stays `... must be positive integers.` MATLAB ends `... or logical
  values.`, which would promise something this interpreter does not do until
  cycle 03 makes logical indexing real. It is scheduled to 03 for that reason,
  where the sentence becomes true at the same moment it is printed.

### The `.err` substring and the exit code are two rules, not one

Acceptance test 7 asks a REPL case to prove three things at once: the
diagnostic is on stderr, it is *not* on stdout, and the session lives through
it and exits cleanly. The harness could express only the middle one. `.err`
meant "stderr contains this substring **and** the process exits 1", a single
rule, so a session that ends in `exit` — exit 0, which is precisely the claim
that it survived — could not carry an `.err` at all. Both REPL cases were
therefore written with an `.out` alone, pinning the absence of the text from
stdout and nothing about its presence anywhere: **a build that swallowed the
diagnostic entirely would have passed them both**, and `repl_error_has_no_line`
would have stopped testing the line prefix it is named for.

So the two rules are separated. `.err` now says only "stderr must contain this
substring". The exit code stays asserted exactly in every case — that is the
tripwire that keeps a panic's 101 and an abort's 134 from passing for a clean
1, and this cycle of all cycles depends on it — but the expected code is now a
value rather than a consequence: implied as before (1 with an `.err`, 0
without), and stated outright in an optional `.exit` file where the implication
is wrong. The two REPL cases are its only users: each gains an `.err` holding
the full `Error: …` text and an `.exit` of `0`.

Starting the substring at the `Error:` prefix is what carries
`repl_error_has_no_line`: an inserted `Line 1: ` would split the prefix from
the message and break the match, so the case tests its own name again. This is
the "move it to the `.err`" that the Scope asks for; the change to a `.out`
outside this module's directory is the one the Scope sanctions, and the commit
body says so.

### Smaller decisions

- **`break` and `continue` are runtime errors, not parse errors.** Octave
  rejects them at parse time, but acceptance test 4 requires `disp(1)` to have
  printed its `1` before the error, which only a runtime check gives.
  `Interp.loop_depth` counts running loops and is restored on every exit path,
  including an error unwinding out of a loop body, so a loop that has ended
  never makes a later top-level `break` legal.
- **Chained ranges: the parser only.** `parse_range` loops, so `1:2:3:4` is
  `(1:2:3):4` and a further colon folds around what has been built — the same
  rule Octave's grammar applies, which also settles `1:2:3:4:5` as
  `(1:2:3):4:5`. Evaluating it then meets SplatCrab's existing refusal of a
  colon start that is not a scalar, so both spellings give the same error,
  which is what acceptance test 2 asks for. MATLAB is understood to take the
  first element of a non-scalar colon operand; that was not verified against a
  real MATLAB run, so it is a new "verify first" row in Known bugs rather than
  a guess encoded in a test.
- **A non-UTF-8 file is decoded leniently.** `main` reads bytes and uses
  `String::from_utf8_lossy`, so a Windows-1252 comment runs instead of
  refusing the file, and `Cannot read` now carries the `Error:` prefix. A
  UTF-16 file still decodes to replacement characters; reading it needs
  encoding detection, not a lossy decode, and it stays in Known bugs. The
  byte-order mark is skipped in `lexer::scan` rather than in `main`, so the
  REPL gets it too, and only a *leading* one is skipped.
- **The REPL's end of input.** A leftover buffer means `needs_more` saw an
  opener that never closed, so nothing in it is run: the statements inside an
  unclosed block were never complete. It reports
  `error::unterminated_block` and exits 1.
- **`s(:)` gets the shape, not the class.** A `Value::Str` is a row of
  characters with nowhere to record another shape, so a result that is not one
  row comes back as its character codes. Fixing the shape here and the class in
  cycle 02 is the split the Out of scope section draws for the neighbouring
  char row; inventing a temporary shaped-string mechanism to delete in 02 is
  the thing both are avoiding. Recorded against QA D17.
- **`''` is `0x0` in `Value::into_mat`.** Every shape query goes through that
  one conversion, so `size('')` and `size(num2str([]))` both follow from it
  rather than from a special case in `size`.
- **`disp([])` prints nothing in `disp`, not in `Matrix::format`.** The named
  display `x = []` still shows `[]`; that is a separate Known deviation
  scheduled to cycle 02, and changing `format` would have moved it too.

## Acceptance tests

One golden case per bullet in `tests/cases/01e-display-and-parser/`.

1. A script with about 200,000 nested parentheses → a clean error naming the
   nesting depth, **exit 1, not 134**. One case each for nested brackets,
   nested calls and a long flat sum.
2. `disp(1:2:3:4)` → the same as `disp((1:2:3):4)`.
3. `y = x + ;` → an error naming `';'`, not `Semi`. A second case for an
   identifier and a number.
4. `disp(1)`, `break`, `disp(2)` → prints `1`, then an error about `break`
   outside a loop, exit 1. Same for `continue`.
5. `if 0` / `elseif undefined_name` / `end` → the error names line 2.
6. `disp(size(1:Inf:5))` → `     1     1`.
7. A `.repl` case: an error in the REPL goes to stderr and not stdout, and the
   session survives. This replaces the existing `repl_error_has_no_line`
   expectations; the old case moves its error text from `.out` to `.err`.
8. A `.repl` case: `for k=1:3` and `disp(k)` with no `end`, then EOF → an
   unterminated-block error, exit 1.
9. A file beginning with the bytes `EF BB BF` then `disp(1)` → `     1`,
   exit 0. Two more pin the recorded non-UTF-8 policy, which is a behaviour
   change and so needs cases of its own: a file carrying a Windows-1252 byte
   in a comment now runs (`     1`, exit 0) instead of being refused, and a
   UTF-16 file now fails *inside* the `Error:` format with exit 1 instead of
   outside it. The UTF-16 case pins the reporting, not the reading, which
   stays in Known bugs.
10. `linspace(1, 2)` → wrapped into `Columns N through M` blocks.
11. `disp([1 2 NaN])` → `     1     2   NaN`; `disp([1 Inf])` likewise.
12. `disp(size(find([])))` → `     0     0`; the same for `diag([])`,
    `size('')`, and `s(:)` on a char giving a column; `disp([])` prints
    nothing.
13. `[1 1] && 1` → an error about a logical scalar, exit 1. Same for
    `[] || 1`.
14. `if NaN, end` → `NaN's cannot be converted to logicals.`, exit 1. Same for
    `NaN & 1` and `~NaN`.
15. The error-text policy: one case pinning the chosen wording of the undefined
    name message, whichever was chosen.
16. Unit: the depth limit fires at the boundary and one past it, in the parser
    and in the evaluator, without touching the stack.

## Status

Done (2026-09-28)
