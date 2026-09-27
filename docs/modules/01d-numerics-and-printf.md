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

Filled in during implementation. Record in particular:

- Where `check_shape` is called from for each of the four result-size paths,
  and whether any of them needed a different signature.
- The exact pivot rule chosen for `solve`, and the norm it is relative to.
- How the colon's end point is computed, and whether `linspace` needed the same
  treatment or only its last element pinned.
- The bound chosen for `printf` width and precision, and what an absurd value
  now reports.
- The error text for an unsupported complex result, which cycle 10 will later
  replace with a value.

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

Planned
