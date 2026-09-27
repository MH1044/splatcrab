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

Filled in during implementation. Record in particular:

- The depth limit chosen, for the parser and for the evaluator, how it was
  arrived at, and what it reports. A limit that is too low breaks legitimate
  deeply nested code; too high and it never fires before the stack does. State
  the margin against the 256 MB stack.
- The human display form for `Token`, and whether any message reads worse for
  it.
- The column-wrapping rule: terminal width assumed, and what happens when the
  output is piped rather than shown on a terminal.
- The error-text policy decided for QA D33, with the MATLAB release named.

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
   exit 0.
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

Planned
