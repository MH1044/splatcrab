# 09 — Numerics

## Goal

`polyfit polyval roots(real) interp1 trapz cumtrapz diff fzero fminsearch integral ode45 odeset conv deconv filter std var median mode factorial nchoosek primes isprime gcd lcm unique ismember setdiff intersect union logspace meshgrid histc`

After this module a user can do things that were not possible before; every
bullet in Scope must be demonstrable by at least one acceptance test below.

## Scope

- `polyfit polyval roots(real) interp1 trapz cumtrapz diff fzero fminsearch integral ode45 odeset conv deconv filter std var median mode factorial nchoosek primes isprime gcd lcm unique ismember setdiff intersect union logspace meshgrid histc`
- **`sort` of a matrix**, the half of the Known deviations row "`sort`
  accepts vectors only" scheduled here: along the columns by default, along
  a dimension with `sort(A, dim)`, with `'descend'`, and `[s, i] = sort(A)`
  giving each column's permutation. Cycle 08 took the `norm` half
- **Every solver calls back through `Interp::call_nested`** (cycle 05's
  rule): `fzero`, `fminsearch`, `integral` and `ode45` evaluate a user
  function many times, and each evaluation counts against the shared
  nesting budget. Each also has a cap that ends in a clean error, never a
  hang (invariant 6): `fzero`'s bracket search and iterations,
  `fminsearch`'s iterations and evaluations, `integral`'s subdivisions, and
  `ode45`'s step count and minimum step. The texts are SplatCrab's own
  unless a source settles MATLAB's
- **Complex results stay refusals until cycle 10**, which the project's
  owner decided on 2026-09-28 to build: `roots` of a polynomial with
  complex roots, `roots([1 0 1])`, is a clean error beginning `Complex
  results are not supported.`, cycle 01d's shared prefix
- **Cells of char** in `unique`, `ismember`, `setdiff`, `intersect` and
  `union`, since cycle 07 made cells: MATLAB programs use them on cell
  arrays of names constantly. Sorted by character code, as `sort` sorts
  chars. Other cell contents are refused with a clean error

Planning added the four bullets above: the `sort` half of a Known
deviations row, cycle 05's callback rule and invariant 6 for solvers, the
complex refusal, and the cells that cycle 07 made possible.

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.
- Anything returning complex results; those must error clearly until cycle 10.
- Stiff solvers such as ode15s.

## Design notes

### Files and types

- **`src/builtins/numerics.rs`** (new): the builtins on data, `polyfit
  polyval roots conv deconv filter interp1 trapz cumtrapz diff std var
  median mode factorial nchoosek primes isprime gcd lcm logspace meshgrid
  histc`, and the helpers the rest share: `first_dim` (MATLAB's default
  dimension, the first that is not a singleton), `extent`, and
  `map_slices`, which hands every column, or every row through a transpose,
  to a closure and judges the result's shape with `check_shape` first.
  Pure kernels are `pub` for their unit tests: `horner`, `convolve`,
  `df2t`, `interp_at`, `differences`, `variance`, `median_of`, `mode_of`,
  `fact`, `binomial`, `sieve`, `is_prime`, `gcd_of`, `lcm_of`, `pow10`,
  `bin_of`.
- **`src/builtins/sets.rs`** (new): `unique ismember setdiff intersect
  union` over a private `Set`, either `Num(Matrix)`, compared as numbers,
  or `Text`, the code units of a cell of character vectors, with the values
  kept to hand back unchanged.
- **`src/builtins/solvers.rs`** (new): `fzero fminsearch integral ode45
  odeset`, each a thin builtin around a pure method that takes the user's
  function as a Rust closure and its caps as parameters: `fzero_solve`
  (the search, then `brent`), `nelder_mead`, `quad` (over `gk15`) and
  `dopri` (over `ntrp45`), with `OdeOptions`. The builtins' closures call
  the function through `Interp::call_nested`; a handle or a function's name
  is accepted, as `feval` accepts either.
- **`src/builtins/linalg.rs`**: `sort` takes a matrix. Each slice along the
  dimension (each column by default, each row with `dim` 2) is sorted by
  the same `sort_cmp` as before, so `NaN` placement and stability in both
  directions are cycle 01c's, and the class is kept as cycle 02 has it.
  The second output is each slice's permutation.
- **`src/builtins/mod.rs`**: the three modules registered; the registry
  grows from 130 to 163.
- **`src/error.rs`**: `sort_vectors_only` is gone. New: `complex_roots`,
  `arg_not_a_vector`, `sample_length`, `arg_nonneg_int`, `arg_integers`,
  `nchoosek_k`, `leading_zero`, `conv_shape`, `interp_method`,
  `interp_extrap`, `interp_points`, `histc_edges`, `std_weight`,
  `set_cell_contents`, `set_mixed`, `set_option`, `solver_output`,
  `solver_nonfinite`, `fzero_no_sign_change`, `fzero_endpoints`,
  `fzero_start`, `integral_limit`, `ode_step_limit`, `ode_min_step`,
  `ode_tspan`, `unrecognized_option`, `option_pairs` and `option_value`.
  Every text is SplatCrab's own. `fzero_endpoints` follows MATLAB's
  wording as recalled, which no source here confirms, so it is SplatCrab's
  own too (relabelled at review). The caps of `fzero` and `fminsearch`
  reuse cycle 08's `no_convergence` (`'fzero' did not converge within its
  iteration limit.`), and `polyfit` reuses its `rank_deficient_warning`.
  The message scan in `error.rs` covers the three new files.

### Invariants

Column-major throughout: a matrix is read and written through `get`/`set`
or `data[c * rows + r]`, a slice along dimension 1 is a column's contiguous
run, and `ode45` builds `y` with one row per time. No builtin here indexes
with user subscripts, so the one-based boundary in `eval_index_args`, the
`end` stack and name resolution are untouched; the one-based index outputs
(`[s, i] = sort`, `ia`, `ic`, `loc`, `bin`) are made one-based where they
are built. Every result shape computed from operands goes through
`check_shape` before it is allocated (`map_slices`, `polyfit`'s
Vandermonde, `nchoosek(v, k)`'s combinations, counted exactly in `u128`
first, the `primes` sieve, `meshgrid`, `histc`, `interp1` of a matrix, and
`ode45`'s output before each step adds its points, so no `Refine` can make
it grow without bound). Output leaves only through `emit` and `emit_err`:
`polyfit`'s warning goes through `Interp::warn`.

Invariant 6: every solver evaluation is one `call_nested`, so it counts
against the shared nesting budget, and a function that calls a solver on
itself meets the recursion limit of 500, a clean error. Every iteration has
a cap that is a clean error, exit 1:

| Solver | Cap | Error |
|---|---|---|
| `fzero`, search | a point or value that stops being finite, which the `sqrt(2)`-growing step reaches in about 2,050 steps from a start near 1; 4,096 steps as the cap, which a start so small that its step underflows to `0` meets | `'fzero' found no sign change of the function in its search for an interval.` |
| `fzero`, Brent | 1,000 iterations | `'fzero' did not converge within its iteration limit.` |
| `fminsearch` | `200 * numel(x0)` iterations and as many evaluations, MATLAB's defaults | `'fminsearch' did not converge within its iteration limit.` |
| `integral` | 650 subintervals (MATLAB's `MaxIntervalCount`), or one too narrow to halve: a half whose outermost Kronrod nodes would round onto its ends | `'integral' reached its limit of 650 subintervals without meeting the tolerance; the integral may not exist.` |
| `ode45` | 50,000 step attempts; a step below `16 * eps(t)` | `'ode45' reached its limit of 50000 steps before the end of the time span.`, `'ode45' cannot meet the tolerances without a step below the smallest allowed, at t = <t>.` |

So acceptance item 15's two terminations both end in exit 1:
`fzero(@(x) x^2 + 1, 0)` with the no-sign-change text, after about 1,030
search steps (at `|x|` near `1.3e154`, `x^2` overflows), and
`integral(@(x) 1 ./ x, 0, 1)` with the subinterval text, after 650. Each
takes about a quarter of a second in the debug build, most of it start-up.

### Choices where the spec was silent

- **`polyfit`** gives `p` only; `[p, S, mu]` is not provided. Too few
  distinct points for the degree warns with `\`'s `Matrix is rank deficient
  to working precision (rank r).` and returns the basic solution, where
  MATLAB warns `Polynomial is not unique; degree >= number of data points.`
- **`roots`** returns a column, the companion matrix's eigenvalues in the
  order `eig_general` deflates them, then a `0` per trailing zero
  coefficient; leading zeros are dropped; a constant or empty `p` is `0x1`.
  A `NaN` or `Inf` coefficient is `eig`'s `Input to 'roots' must not contain
  NaN or Inf.` A real root of multiplicity three or more, `roots([1 -6 12
  -8])`, comes out of the eigensolver as a complex pair a rounding error
  apart, as MATLAB's does (MATLAB prints `2.0000 + 0.0000i`), so it is the
  complex refusal until cycle 10. A double root, `roots([1 -2 1])`, is real.
- **`conv`** takes `'full'`, `'same'` (the central part, starting at
  `floor(numel(v) / 2)`, checked against the MATLAB page's example) and
  `'valid'`. The result is a column when `u` is one, or when `u` is a scalar
  and `v` a column; an empty operand gives `[]`. **`deconv`** gives `q` in
  `b`'s orientation, `0` when the divisor is longer, and `r` in `b`'s shape
  with its leading coefficients exactly `0`.
- **`filter(b, a, x)`** only: no `zi`, `zf` or `dim`. It works along the
  first non-singleton dimension. An empty `b` is the zero filter.
- **`interp1`** methods: `'linear'` (default), `'nearest'`, `'previous'`
  and `'next'`; `'pchip'`, `'spline'`, `'cubic'` and the rest are the
  method error. The fifth argument is `'extrap'` (the end interval's line
  for `'linear'`, the nearest end value for the others) or a scalar. The
  sample points are sorted with their values, and must be at least two,
  distinct and not `NaN`. A query exactly on a point returns its value; a
  `'nearest'` tie takes the upper point. A matrix `v` interpolates each
  column, `numel(xq)` rows; a matrix `xq` with it is the N-D refusal.
- **`trapz` and `cumtrapz`**: `(y)`, `(x, y)`, `(y, dim)` and `(x, y,
  dim)`, two arguments being `(y, dim)` when the second is a scalar, as
  MATLAB decides; `x` is a scalar spacing or a vector of points. Fewer than
  two samples integrate to `0`.
- **`diff(X, n, dim)`**: without `dim`, each of the `n` rounds works along
  the first non-singleton dimension of what the last left; with it, all of
  them along `dim`, which ends at `max(size(X, dim) - n, 0)`. `diff(5)` is
  `0x1`. Along a dimension past the second with `n > 0` the result would be
  `rows x cols x 0`, the N-D refusal.
- **`std`, `var`**: weight `0` or `[]` for `N - 1`, `1` for `N`; a weight
  vector is refused. A dimension is the third argument. One element has
  variance `0`, none `NaN`. **`median`, `mode`**: a dimension second, no
  `'omitnan'`. `[M, F] = mode(...)` gives the count; `mode` of nothing but
  `NaN` is `NaN` with a count of `0`. Along a dimension past the second,
  each element is its own slice. All four return doubles.
- **`factorial`**: `NaN` and `Inf` pass through; past `170!` it is `Inf`.
  **`nchoosek(n, k)`** is the multiplicative formula over `min(k, n - k)`,
  exact below 2^53, without MATLAB's inexactness warning above it;
  **`nchoosek(v, k)`** lists combinations in the lexicographic order of
  positions, in `v`'s class. **`primes(n)`** below 2, `NaN` included, is
  `1x0`. **`isprime`** is Miller-Rabin with the first twelve prime bases,
  deterministic for every `u64`; every double from 2^53 is even. **`gcd`**
  and **`lcm`** take finite integers of either sign and return non-negative
  values, `lcm` `0` when either is.
- **`logspace(a, b, n)`**: `n` as `linspace` reads it, 50 by default, the
  exponents placed as `linspace` places them, `10^k` exact for integral
  `|k| <= 22`; a `b` of exactly `pi` ends at `pi`. **`meshgrid`**: two
  inputs at most, a third is the N-D refusal; `Y` only when asked for.
- **`histc(x, edges)`**: counts follow `x`'s orientation for a vector and
  are one column per column for a matrix; `[n, bin]`; no `dim`. The edges
  must be non-decreasing with no `NaN`.
- **Sets**: index outputs `[C, ia, ic] = unique`, `[C, ia] = setdiff`,
  `[C, ia, ib] = intersect` and `union`, and `[tf, loc] = ismember`, every
  index a column, `ia` the first occurrence, `loc` the lowest index. The
  options are `'sorted'` and `'stable'` (`'rows'` and `'legacy'` are the
  option error). Orientation: `unique` gives a row for a row vector and a
  column otherwise, a matrix and `{}` included; `setdiff` a row when `A` is
  a row, or `[]` beside a row `B`; `intersect` and `union` a row when both
  inputs are rows or `[]`, not both `[]`. An array result keeps the class
  both inputs share, and is a double otherwise. A cell element must be a
  character row or `''`; a character vector beside a cell is one text, and
  two arrays, chars included, are compared element by element.
- **`fzero`**: `(f, x0)` and `(f, [a b])` only, no options. The search is
  MATLAB's: steps of `x0/50` (`1/50` from `0`), growing by `sqrt(2)`,
  alternately below and above `x0`. Brent's method stops at `2 eps |x| +
  eps/2`. A zero found exactly, at the start or an end point, is returned
  as is. `[x, fval, exitflag]`, the flag always `1`. A value that is not
  finite at the start, or at either end of a bracket, is `The function
  passed to 'fzero' returned NaN or Inf.`; one met during the search for an
  interval ends the search instead, with the no-sign-change error of the cap
  table below (`fzero(@(x) 1 + 0/(x < 3), 0)` is that error). Reconciled at
  review, where the two statements disagreed
- **`fminsearch`**: `(f, x0)` only, no options; MATLAB's initial simplex
  (5%, or `0.00025` for a zero coordinate), coefficients 1, 2, 1/2, 1/2 and
  tolerances `TolX = TolFun = 1e-4`. `x` and every point `f` sees have
  `x0`'s shape. `[x, fval, exitflag]`, the flag `1`.
- **`integral`**: global adaptive Gauss-Kronrod 7-15 (QUADPACK's nodes and
  weights, the error `|K15 - G7|`), ten equal subintervals to start, the
  worst one halved until the errors sum to `max(AbsTol, RelTol * |Q|)`,
  `AbsTol` 1e-10 and `RelTol` 1e-6 by default and as name-value pairs.
  Infinite limits are mapped onto a finite interval: `x = a + t/(1 - t)`
  on `[0, 1)` for `[a, Inf)`, `x = b - t/(1 - t)` for `(-Inf, b]`, and
  `x = t/(1 - t^2)` on `(-1, 1)` for the whole line, each weighted by its
  derivative; no node is an end point. `b < a` negates, `a == b` is `0`, a
  `NaN` limit is `NaN`. `f` is called with a row of 15 points and must
  return a value for each; a `NaN` or `Inf` value is an error.
  `'ArrayValued'` and `'Waypoints'` are the unrecognised-option error.
- **`ode45`**: the Dormand-Prince 5(4) pair and step control of MATLAB's
  `ode45` (the scaled infinity norm of the error estimate against
  `RelTol`, growth at most 5x, a first failure shrinking by the error's
  fifth root and at least 10x, later ones halving; `MaxStep` a tenth of
  the span; MATLAB's initial step), and its continuous extension for
  `Refine` (4 by default for a two-element `tspan`) and for a longer
  `tspan`, which is output exactly. `RelTol` below `100 * eps` is raised to
  it silently, where MATLAB warns. `y` is passed to `f` as a column, and
  `f` must return `numel(y0)` values. A `NaN` error estimate is a failed
  step, so a blow-up ends at the minimum step. Asked for one output,
  `ode45` returns a struct with `solver`, `x` (a row of times) and `y` (a
  column per time), not MATLAB's full solution struct.
- **`odeset`** knows five options, `AbsTol InitialStep MaxStep Refine
  RelTol`, stored in that order, matched in any case; any other name is
  refused, as is a value that is not positive and finite (`AbsTol` may be a
  vector, `Refine` must be an integer). `odeset(old, ...)` starts from a
  struct. `odeset()` returns every field empty rather than printing the
  list. `ode45` reads any struct, matching fields in any case and ignoring
  the ones it does not know.

### Settled in testing

- **Item 15's exit codes** are 1 for both terminations, each with its text
  pinned: `err_fzero_no_sign_change` and `err_integral_divergent`. The
  cases that asserted an exit code alone now pin their texts too, all
  SplatCrab's own: `err_fminsearch_unbounded` (cycle 08's
  `no_convergence`), `err_ode45_blowup`, `err_unique_cell_of_numbers`,
  `err_ismember_cell_of_numbers`, and `err_primes_huge_bound`, whose sieve
  is judged by `check_shape`, so its text is the size error's (`Requested
  1x1000000000000 array exceeds the maximum array size.`).
- **The time in `ode45`'s minimum-step text** is where the numerical
  solution blows up, which depends on the step sequence and so on the last
  bits of `pow`. `y' = y^2` from `1` over `[0 2]` stops at `t = 0.99997`
  (the numerical pole, a little before the true one at 1), and
  `err_ode45_blowup` pins four digits, `at t = 0.9999`, no more.
- **Every new message has an `err_*` case**, named in the items above; the
  step cap has `err_ode45_step_limit`, which reaches its 50,000 attempts in
  about 1.3 s of the debug build by calling the builtin `max` by name with
  `Refine` 1. Brent's own cap, `'fzero' did not converge within its
  iteration limit.`, cannot be reached from a script (a bracket with a
  sign change closes by bisection first) and has a unit test only.
- **`integral` never evaluates an end point.** Halving towards an infinite
  limit's pole used to go on until a node rounded onto it, where `x` is
  infinite, and so ended in `The function passed to 'integral' returned
  NaN or Inf.` for a function that returned `1`. A half whose outermost
  nodes would round onto its ends is now "too narrow to halve", the
  subinterval error: `err_integral_divergent_infinite_range`.
- **A function returning `NaN` or `Inf` everywhere** in `fminsearch` shrinks
  the simplex onto `x0`, since every comparison with `NaN` is false, and
  `x0` is returned with exit 0, as MATLAB's comparisons do. Not an error:
  the spec names no cap for it and the method ends on its own.
- **`histc(x, [])`** is empty in `x`'s orientation, `1x0` for a row, one
  count per edge: `histc_empty_edges`. **`integral(f, Inf, Inf)`** is `0`,
  the `a == b` rule before the `NaN` one.
- **Costs with no cap to meet**, each a fixed trip count and so outside
  invariant 6: `conv` is the direct O(`numel(u) * numel(v)`) sum, so two
  `1e6`-element vectors (`1e12` products) take hours in the debug build
  (`1e4` by `1e4` takes 2.7 s, `3e4` by `3e4` 41 s); `primes(1e8)` takes
  about 5 s in the debug build; `nchoosek(1000, 500)` is finite,
  `2.7029e+299`, not `Inf`: `nchoosek_1000_500_finite`.

### Deviations accepted

Recorded in the Known deviations row "Numerics, cycle 09" of
`docs/ARCHITECTURE.md`: a failing solver is an error where MATLAB warns
and returns what it has (a `NaN` from `fzero`, the last simplex from
`fminsearch`, an estimate from `integral`, a partial solution from
`ode45`); `polyfit`'s rank warning text; `roots` of a triple root; the
one-output `ode45`; and the texts, which are SplatCrab's own.

**Fixed at review.** Three findings:

- `fminsearch` built its `(n + 1) x n` simplex without `check_shape`, so
  `x0 = zeros(1, 1e5)` asked for about 80 GB and ended in an allocator
  abort (134) instead of a clean error. The shape is now judged before the
  simplex is built or `f` is called (`err_fminsearch_huge_simplex`).
- Three message texts were passed into `error::solver_output` from
  `solvers.rs`; they are now named constructors in `src/error.rs`, so every
  text lives there. No `.err` changed.
- The completion lists pinned by two U0 cases now name `diff`, the builtin
  this cycle added between `diag` and `disp` (reason N3 in the commit).

Noted, not fixed: `ode45`'s `Refine` is bounded only by the global element
cap, so `Refine` 1e8 builds about 2e8 output points before the size error;
`fminsearch`'s cost grows as `n^3` at MATLAB's default caps (100 dimensions
reach the cap in 12 s in the debug build); `conv` is the direct sum, `1e12`
multiply-adds for two `1e6` vectors.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/09-numerics/`. Expected output is written by hand from MATLAB
semantics wherever feasible; the `.err` substring is always chosen by hand.
Every new error message needs an `err_*` case.

1. `p = polyfit([1 2 3], [2 4 6], 1); fprintf('%.4f %.4f\n', abs(p)); disp(polyval([1 0 -1], 3))` → `2.0000 0.0000\n     8` Cases: polyfit_line_fit, polyval_scalar, err_polyval_matrix_coefficients, err_polyfit_length_mismatch, polyfit_too_few_points_warns.
2. `r = sort(roots([1 -3 2])); fprintf('%.4f %.4f\n', r)` → `1.0000 2.0000` Cases: roots_real_distinct.
3. `disp(interp1([1 2 3], [10 20 30], 2.5)); disp(interp1([1 2 3], [10 20 30], 0.5)); disp(interp1([1 2 3], [10 20 30], 2.4, 'nearest'))` → `    25\n   NaN\n    20` Cases: interp1_linear_inside, interp1_outside_is_nan, interp1_nearest, err_interp1_spline_method, err_interp1_bad_extrap, err_interp1_repeated_points, err_interp1_length_mismatch.
4. `disp(trapz([1 2 3])); disp(trapz([0 1 2], [0 1 4])); fprintf('%.1f ', cumtrapz([1 2 3])); fprintf('\n'); disp(diff([1 4 9 16])); disp(diff([1 4 9 16], 2)); disp(diff([1 2; 4 8]))` → `     4\n     3\n0.0 1.5 4.0 \n     3     5     7\n     2     2\n     3     6` Cases: trapz_unit_spacing, trapz_with_x, cumtrapz_unit_spacing, diff_vector, diff_second_order, diff_matrix_columns, err_diff_negative_order, err_trapz_length_mismatch.
5. `fprintf('%.6f\n', fzero(@(x) x^2 - 2, 1)); fprintf('%.6f\n', fzero(@(x) cos(x) - x, [0 1]))` → `1.414214\n0.739085` Cases: fzero_scalar_start, fzero_bracket, err_fzero_nan_value, err_fzero_vector_value, err_fzero_endpoints_same_sign, err_fzero_three_element_start, err_fzero_function_errors.
6. `x = fminsearch(@(x) (x(1) - 1)^2 + (x(2) - 2)^2, [0 0]); disp(norm(x - [1 2]) < 1e-3)` → `   1`. Rewritten at planning as a self-check: a search that stops at its tolerance can land either side of a `%.3f` boundary Cases: fminsearch_quadratic_bowl.
7. `fprintf('%.4f\n', integral(@(x) x.^2, 0, 1)); fprintf('%.4f\n', integral(@(x) exp(-x.^2), -Inf, Inf))` → `0.3333\n1.7725` Cases: integral_finite_interval, integral_infinite_limits, err_integral_scalar_valued, err_integral_nan_value, err_integral_unknown_option.
8. `[t, y] = ode45(@(t, y) -y, [0 1], 1); disp(abs(y(end) - exp(-1)) < 1e-3); [t, y] = ode45(@(t, y) [y(2); -y(1)], [0 pi], [0; 1]); disp(abs(y(end, 2) + 1) < 1e-3)` → `   1\n   1`. Rewritten at planning as self-checks: `ode45`'s default tolerances allow errors near `1e-3`, so `%.3f` of `exp(-1)` could print `0.367` or `0.368` Cases: ode45_scalar_decay, ode45_oscillator_system, ode45_output_columns, odeset_tolerances, ode45_reversed_tspan, err_ode45_equal_times, err_ode45_wrong_length_value, err_odeset_unknown_option, err_odeset_missing_value, err_odeset_negative_tolerance.
9. `disp(conv([1 2], [1 3])); fprintf('%.2f ', filter(1, [1 -0.5], [1 0 0])); fprintf('\n'); disp(filter([1 1] / 2, 1, [2 4 6]))` → `     1     5     6\n1.00 0.50 0.25 \n     1     3     5` Cases: conv_polynomials, filter_iir_impulse, filter_fir_moving_average, deconv_quotient_remainder, err_filter_leading_zero, err_conv_bad_shape.
10. `fprintf('%.4f %.4f\n', std([1 2 3 4]), var([1 2 3 4])); disp(median([3 1 2])); disp(mode([1 2 2 3])); disp(factorial(5)); disp(nchoosek(5, 2)); disp(gcd(12, 18)); disp(lcm(4, 6)); disp(primes(20))` → `1.2910 1.6667\n     2\n     2\n   120\n    10\n     6\n    12\n     2     3     5     7    11    13    17    19` Cases: std_var_sample, median_odd_count, mode_most_frequent, factorial_five, nchoosek_five_two, gcd_scalars, lcm_scalars, primes_to_twenty, isprime_row, factorial_overflow_inf, err_primes_huge_bound, err_gcd_non_integer, err_gcd_nan, err_factorial_negative, err_nchoosek_k_above_n, nchoosek_1000_500_finite, err_std_bad_weight.
11. `disp(unique([3 1 2 1])); [tf, loc] = ismember([2 5], [1 2 3]); disp(tf); disp(loc); disp(logspace(0, 2, 3)); [X, Y] = meshgrid(1:2, 1:3); disp(size(X))` → `     1     2     3\n   1   0\n     2     0\n     1    10   100\n     3     2` Cases: unique_sorted_row, ismember_tf_loc, logspace_decades, meshgrid_size, meshgrid_grid_values, intersect_sorted_row, union_sorted_row, setdiff_sorted_row, histc_bin_counts, err_meshgrid_huge_size, err_logspace_huge_count, err_histc_decreasing_edges, histc_empty_edges, err_unique_rows_option.
12. `disp(sort([3 1; 2 4])); disp(sort([3 1; 2 4], 2)); [s, i] = sort([3 1; 2 4]); disp(i); disp(sort([3 1; 2 4], 'descend'))` → `     2     1\n     3     4\n     1     3\n     2     4\n     2     1\n     1     2\n     3     4\n     2     1` Cases: sort_matrix_columns, sort_matrix_along_rows, sort_matrix_permutation, sort_matrix_descend.
13. `roots([1 0 1])` → err containing `Complex results are not supported.`, exit 1 Cases: err_roots_complex.
14. `u = unique({'b', 'a', 'b'}); disp(numel(u)); disp(u{1}); disp(ismember('a', {'a', 'b'})); d = setdiff({'a', 'b', 'c'}, {'b'}); disp([d{:}])` → `     2\na\n   1\nac` Cases: unique_cellstr, ismember_char_in_cellstr, setdiff_cellstr, unique_cellstr_char_code_order, ismember_cellstr_two_outputs, intersect_cellstr, union_cellstr, err_unique_cell_of_numbers, err_ismember_cell_of_numbers, err_ismember_number_in_cellstr.
15. Termination: `fzero(@(x) x^2 + 1, 0)` (no sign change to find) and `integral(@(x) 1 ./ x, 0, 1)` (divergent) each end with exit 0 or 1, never 101, 134 or a hang, the case asserting the exact code the implementation chose and recorded; any text asserted is SplatCrab's own, pinned in its `err_*` case. And `disp(abs(fzero(@(x) fzero(@(y) y - x, 0), 1)) < 1e-6)` → `   1` shows a solver calling a solver, nested through `call_nested` (the root is 0; a self-check, since the raw value is roundoff) Cases: err_fzero_no_sign_change, err_integral_divergent, fzero_nested_in_fzero, err_fminsearch_unbounded, err_ode45_blowup, err_fzero_recursion_limit, err_integral_recursion_limit, err_fminsearch_recursion_limit, err_ode45_recursion_limit, err_integral_divergent_infinite_range, err_ode45_step_limit.

## Status

Done (2026-09-29)
