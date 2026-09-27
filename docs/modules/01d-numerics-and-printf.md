# 01d — Numerics and printf

## Goal

Close the numeric and `printf` defects, and with them three of the four inputs
that still abort the process. After this cycle only one member of that family
is left, and it belongs to 01e.

```matlab
[Inf 0] * [0; 1]          % NaN, not 0
[1e-15 0; 0 1e-15] \ [1; 1]   % solves; the matrix is perfectly conditioned
x = 0:0.1:0.3; x(end) == 0.3  % 1
fprintf('%.65536f', 1)    % a clean error, not a panic
sqrt(-4)                  % a clear error until complex numbers land
```

## Scope

Fifteen rows in the Known bugs table of `docs/ARCHITECTURE.md` are marked 01d.
Fourteen are here; the fifteenth is resolved without a change (see Out of
scope). Each bullet names its row.

**The aborts, first and before anything else.** These break invariant 6, and
the cycle is not worth starting if they are left to the end.

- **Result sizes are never checked** (QA D1). `ones(1e5,1) + ones(1,1e5)`
  aborts in the allocator with exit 134 and takes the REPL with it. The same
  goes for `ones(1e5,1) * ones(1,1e5)` and for a two-subscript index read with
  large index vectors. Every place that computes a result shape from operand
  shapes must go through `args::check_shape` before allocating: `zip`,
  `matmul`, two-subscript `index_read`, and `reduce`.
- **The `matmul` result size wraps** (QA D2). `zeros(2^32,0) * zeros(0,2^32)`
  reports a 4294967296-square result and then panics on transpose, because
  `rows * cols` overflows in `Matrix::filled` and only a debug build catches
  it. Fixed by the same guard.
- **`printf` checks neither width nor precision** (QA D3). `fprintf('%.65536f', 1)`
  panics because Rust holds precision in a `u16`; `fprintf('%.65535e', 1)`
  panics on a separate assertion; `fprintf('%2147483647d', 1)` hangs building
  the pad. Bound every conversion, including the `%g`, `%s` and `%d` fallbacks
  and the width `parse().unwrap_or(0)`. No value may panic or hang; an absurd
  one is a clean error.

**Numerics.**

- **`matmul` swallows `Inf` and `NaN`.** The `if b == 0.0 { continue }`
  sparsity shortcut skips the multiply, so `Inf * 0` and `NaN * 0` never
  happen and `[Inf 0] * [0; 1]` gives `0` where MATLAB gives `NaN`. Remove the
  shortcut. Also listed in cycle 08's Scope; remove it there.
- **`solve` uses an absolute pivot tolerance.** A fixed `1e-14` calls the
  diagonal `[1e-15 0; 0 1e-15]` singular although it is perfectly conditioned.
  Make the threshold relative to the matrix norm, and make `det` and `solve`
  agree on what singular means. Also listed in cycle 08's Scope; remove it
  there.
- **The colon operator and `linspace` miss their end points** (QA D15).
  `x = 0:0.1:0.3; x(end) == 0.3` is false, and `x = -1:0.01:1` is not
  symmetric. MATLAB computes the upper half of a colon from the right-hand end
  point rather than by repeated addition, and `linspace` sets its last element
  to the end point exactly. Do both.
- **A non-finite range end point gives an empty.** `0:Inf` and `-Inf:1:0` are
  `1x0`; MATLAB refuses them and so does Octave 8.4. Refuse them. `1:NaN` is
  left alone: Octave gives a `1x1` `NaN` and MATLAB is unverified, so it stays
  an empty and stays recorded.
- **`trace([])` is negative zero.** `trace` still uses Rust's `.sum()`, whose
  empty sum is `-0.0`. Cycle 01c fixed `sum`, `mean`, `norm` and `dot` through
  `math::sum0` and missed this one. Use `sum0`.
- **The loop variable after a zero-iteration `for`.** After `k = 7; for k = [];
  end`, `k` is still `7`, and a variable that did not exist stays undefined.
  MATLAB assigns the empty. Assign it.

**Results that should be complex** (QA D14).

- `sqrt(-4)`, `log(-1)`, `log2(-8)`, `log10(-10)`, `asin(2)`, `acos(-2)`,
  `(-8)^(1/3)`, `(-8).^(1/3)` and `power(-2, 0.5)` all return `NaN` and exit 0.
  MATLAB returns a complex value. Until cycle 10 exists, each must raise a
  clean error naming complex numbers as unsupported, rather than handing back a
  `NaN` that looks like a computed answer. This is the honest interim: a wrong
  number that says nothing is worse than a refusal.

**printf.**

- **`%d` saturates at 64 bits.** `fprintf('%d', 1e30)` prints the `i64` clamp.
  Print the value.
- **`printf` ignores precision on strings.** `fprintf('[%5.2s]', 'abcdef')`
  gives `[abcdef]`; C and MATLAB truncate before padding, giving `[   ab]`.

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.
- **`printf` conversions and flags differ from MATLAB (QA D16), moved to cycle
  11.** Seven sub-items: `%E` and `%G` printing a lower-case `e`, `%s` of a
  non-integer using `%g` rather than `%e`, the `#` flag, and the rest. They are
  cosmetic differences in a builtin cycle 11 already owns and rewrites for file
  output. This cycle keeps every `printf` panic and hang, which is the part
  that matters, and leaves the spelling to 11. This is the reduction the plan
  anticipated if 01d proved too large, and it did.
- **`mod` and `rem` with an infinite divisor: resolved, no change.** The row
  was marked "verify first". `mod(5, Inf)` and `rem(5, Inf)` give `NaN` here;
  Octave 8.4 agrees, and so do MATLAB's own documented formulas
  `a - m.*floor(a./m)` and `a - b.*fix(a./b)`. Nothing supports the `5` that
  was once suspected. The behaviour is correct, so the row is removed from
  Known bugs with that note. **No golden case may assert either value**, since
  no real MATLAB was run: the evidence is strong enough to stop treating it as
  a bug, not strong enough to pin.
- **`for` over a matrix with no rows (QA D35): still unsettled.**
  `for q = zeros(0,3)` iterates three times here and zero times in Octave, and
  the MATLAB documentation is ambiguous. The row stays open and marked "verify
  first" until someone runs MATLAB.
- Complex numbers themselves. This cycle turns a silent `NaN` into an error;
  cycle 10 turns the error into an answer.
- N-D arrays, and the indexed-growth size message, which cycle 03 owns.

## Design notes

### The four result-size paths

All four call `args::check_shape(rows as f64, cols as f64)` immediately before
the allocation, and after any check that would give a better message:

| Path | Where | Judged before |
|---|---|---|
| broadcasting | `Matrix::try_zip`, `value.rs` | `Vec::with_capacity(rows * cols)` |
| matrix product | `Matrix::matmul`, `value.rs` | `Matrix::filled(self.rows, o.cols, 0.0)` |
| two-subscript read | `index_read`, `interp.rs` | `Vec::with_capacity(rows.len() * cols.len())` |
| reduction | `math::reduce` | `Matrix::row` / `Matrix::col` |

`check_shape` itself did not change, and neither did its signature. It already
took `f64` sizes so that a size past `usize` could be named as asked, and
every dimension arriving here is a `usize` below `2^53`, so the conversion is
exact and the message reads the same as a constructor's.

Two signatures did change. `Matrix::zip` became a thin wrapper over a new
`Matrix::try_zip`, whose closure returns `R<f64>`: the size guard needed the
function to be fallible before its loop, and `.^` needed it fallible inside
the loop for the complex-result error below, so one change served both.
`math::reduce` now returns `R<Matrix>`, which moved `extremum_along` and the
two call sites in `reduction` and `dot` to `?`.

`matmul` also returns early when the result has no elements. `check_shape`
passes a legal 0-by-2^32 result, and the column loop would then spin four
billion times over nothing. That is a hang rather than an abort, so it is not
one of the three, but it is the same family and one line to close.

### The pivot rule

`Matrix::singular_tol` is `f64::EPSILON * n * max |A|`, where `n` is the row
count and the maximum runs over the **finite** entries of the original matrix.
`solve` and `det` both call it once, before elimination, and both test
`pivot.abs() <= tol`.

Three choices worth recording:

- The norm is the largest magnitude, not a 1- or Frobenius norm. It is the
  cheapest quantity that scales with the matrix, and the rule only has to
  separate "small because the matrix is small" from "small because the matrix
  is singular", which any norm does equally well.
- `<=`, not `<`. The all-zero matrix has norm `0` and so tolerance `0`, and
  `0 < 0` is false; it would have been declared non-singular and then divided
  by. `<=` keeps it singular, as the old fixed threshold did.
- Non-finite entries are excluded from the norm. Including them makes the
  tolerance `Inf` or `NaN` for any matrix holding an `Inf`, and every finite
  pivot is then at or below it, so `[Inf 0; 0 1] \ [1; 1]` would report a
  singular matrix where it used to solve. A pivot that is itself `Inf` or
  `NaN` fails the `<=` test and flows into the arithmetic exactly as before.

`det` used to return `0` only for an exactly zero pivot, which is why it and
`solve` disagreed. They now make the identical test and differ only in what
they do about it: `det` reports `0`, `solve` refuses.

### The colon's end point, and `linspace`

`range` keeps the step count it always had, `floor((b - a) / s + 1e-10)`, and
then decides what the right-hand end point is. If that floor discarded no
partial step — `n >= (b - a) / s`, where the `1e-10` fuzz counts as landing —
the end point is `b` itself; otherwise it is `a + n*s`, so `0:0.1:0.35` keeps
`a + 3s` and does not jump to `0.35`.

The vector is then built from both ends: element `k` is `a + k*s` while
`2k <= n`, and `right - (n - k)*s` beyond that. Two properties fall out. The
last element is `right - 0` exactly, so `x = 0:0.1:0.3; x(end) == 0.3` is `1`.
And elements `k` and `n - k` sum to `a + right` exactly, because the same
`k*s` is added to one and subtracted from the other, so `-1:0.01:1` is
symmetric however `k*s` rounds.

`linspace` needed only its last element pinned to `b`. Its interior points are
already one multiply and one add away from `a` rather than a running sum, so
there is no drift for a two-sided computation to cancel; only the final
`(b - a) * (n-1) / (n-1)` was losing the end point.

The non-finite end points needed no new error. `0:Inf` and `-Inf:1:0` both
have the step count `Inf`, and `check_shape(1, Inf)` already reports
`Requested 1xInf array exceeds the maximum array size.`, which is the wording
`0:1e-300:1e300` has given since 01c. The early return that used to swallow
them now fires only on a `NaN` step count, which keeps `1:NaN` the empty it
was — deliberately, since Octave gives a `1x1` `NaN` there and MATLAB is
unverified. `Inf:1:0` stays empty too: its count is negative, as it is for any
range that runs the wrong way.

### The `printf` bound

`core::MAX_FIELD` is **8192**, and it bounds the width and the precision of
every conversion. An absurd value reports
`The width or precision in a format specifier must be at most 8192.`

8192 sits far from both ends of the problem. A double's longest exact decimal
expansion is 1074 fractional digits, so `%.8192f` still prints any value in
full; the lowest limit above is Rust's `u16` precision at 65535, and `%e`
fails one below that on its own `ndigits > 0` assertion. The bound is applied
at parse time, in one helper, `core::field`, so the fallbacks are covered by
construction: `%d` of a non-integer reaches `fmt_e` and `%s` of one reaches
`fmt_g` with a precision that is already bounded. The same helper replaced the
old `parse().unwrap_or(0)`, so a width too long even for a `usize` is now the
same clean error rather than being silently dropped.

One consequence worth naming: because the bound is applied while the specifier
is being read, it fires before the "ran out of data" early return, so
`fprintf('%d %.99999f', 1)` is now an error where it used to print `1 ` and
stop. An absurd field is a defect in the format string whether or not the data
reaches it, so that is the answer this cycle wants. No golden case covered the
old behaviour.

`%d` past `2^63` prints `format!("{:.0}", |v|)`, the exact decimal expansion
of the double, and keeps the `i64` path below that so nothing that already
printed correctly moved. `%.Ns` truncates in `core::truncate` before the width
pads, which is C's order and MATLAB's.

### The complex-result error

Three constructors in `error.rs`, all opening with the same sentence so a
golden case can match on it alone:

- `Complex results are not supported. '<name>' of a negative number is complex.`
  — `sqrt`, `log`, `log2`, `log10`.
- `Complex results are not supported. '<name>' of a value outside [-1, 1] is complex.`
  — `asin`, `acos`.
- `Complex results are not supported. A negative number raised to a fractional power is complex.`
  — `^`, `.^` and `power`.

The domains live in `math::Real`, checked over the whole argument before any
of it is mapped, and in `math::powf_real`, which `interp.rs` uses for both
spellings of the power operator so that `(-8)^(1/3)`, `(-8).^(1/3)` and
`power(-8, 1/3)` cannot drift apart. `NaN` is inside every one of these
domains, because `sqrt(NaN)` is `NaN` in MATLAB too, and an infinite exponent
is inside `powf_real`'s, because `(-2)^Inf` is a real `Inf`. Cycle 10 replaces
each error with a value; until then the refusal is the honest answer.

### What this cycle did not fix

- Deep nesting (QA D4) still aborts with exit 134. It is the last member of
  the process-killing family and it belongs to 01e. **Invariant 6 is not
  restored.**
- `1:Inf:5` is still a `1x0` where MATLAB gives a `1x1`. The D15 row was about
  end points; an infinite *step* is a separate early return, and it is now a
  Known bugs row of its own scheduled to 01e.
- `1:NaN` is still an empty, and now has its own "verify first" row.
- `mod` and `rem` with an infinite divisor were removed from Known bugs with
  no code change and no golden case, per Out of scope.
- `for` over a matrix with no rows (QA D35) is untouched and still open.

## Acceptance tests

One golden case per bullet in `tests/cases/01d-numerics-and-printf/`. Every
abort case must exit **1**, never 101 and never 134: a panic and an allocator
abort are both failures of this cycle's central claim, and a case that only
checks the message would pass on either.

1. `x = ones(1e5, 1) + ones(1, 1e5);` → a clean error naming the size, exit 1.
2. `ones(1e5, 1) * ones(1, 1e5)` → the same, exit 1.
3. `A = [1 2; 3 4]; A(ones(1,1e5), ones(1,1e5))` → the same, exit 1. The
   matrix is tiny; it is the two index vectors that ask for a 1e5-square
   result.
4. `x = zeros(2^32, 0) * zeros(0, 2^32); disp(size(x))` → a clean error rather
   than a wrapped size, exit 1.
5. `fprintf('%.65536f', 1)` → a clean error, exit 1, no panic.
6. `fprintf('%.65535e\n', 1)` → a clean error, exit 1, no panic.
7. `fprintf('%2147483647d', 1)` → a clean error, exit 1, and it returns
   promptly rather than building a two-gigabyte pad.
8. `disp([Inf 0] * [0; 1])` → `NaN`; `disp([NaN 0] * [0; 1])` → `NaN`.
9. `x = [1e-15 0; 0 1e-15] \ [1; 1]; fprintf('%g %g\n', x)` → `1e+15 1e+15`.
   And the singular `[1 2; 2 4] \ [1; 2]` still errors.
10. `x = 0:0.1:0.3; disp(x(end) == 0.3)` → `1`;
    `x = -1:0.01:1; disp(all(x + fliplr(x) == 0))` → `1`;
    `y = linspace(0, 1, 7); disp(y(end) == 1)` → `1`.
11. `0:Inf` → a clean error, exit 1. Same for `-Inf:1:0`.
12. `fprintf('%.4f\n', trace([]))` → `0.0000`, not `-0.0000`.
13. `k = 7; for k = []; end; disp(isempty(k))` → `1`. **Do not assert the exact
    shape**, and use `[]` rather than `1:0`, whose empty shape is not settled.
14. `sqrt(-4)` → a clean error naming complex numbers, exit 1. One case each
    for `log(-1)`, `asin(2)` and `(-8)^(1/3)`; the rest may share a case.
15. `fprintf('%d\n', 1e30)` → the full value, not `9223372036854775807`.
16. `fprintf('[%5.2s]\n', 'abcdef')` → `[   ab]`.
17. Unit: `check_shape` is reached from `zip`, `matmul`, `index_read` and
    `reduce`, each asserted directly rather than only through a script.
18. Unit: the `printf` width and precision bound, at the boundary and one past
    it, for every conversion including the fallbacks.

## Status

Done (2026-09-28)
