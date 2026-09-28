# 08 — Linear algebra

## Goal

LU (reused by `det`/`solve`), Householder QR + least-squares `\` and `/`, Cholesky, Jacobi symmetric eig, Hessenberg+QR real eig, one-sided Jacobi SVD, `rank pinv null orth kron cross triu tril magic cond`, matrix `norm` (1, 2, inf, fro), singular warning instead of error

After this module a user can do things that were not possible before; every
bullet in Scope must be demonstrable by at least one acceptance test below.

## Scope

- LU (reused by `det`/`solve`)
- Householder QR + least-squares `\` and `/`
- Cholesky
- Jacobi symmetric eig
- Hessenberg+QR real eig
- one-sided Jacobi SVD
- `rank pinv null orth kron cross triu tril magic cond`
- matrix `norm` (1, 2, inf, fro)
- Singular systems warn instead of erroring, and so do `inv` and `A^-1` of a
  singular matrix (QA D26), which return `Inf` matrices as MATLAB and Octave
  do: `inv([1 2; 2 4])` is `Inf Inf; Inf Inf`, and `inv(0)` is `Inf`

- Verify first: `det([1 2; 3 4])` is exactly `-2` here and displays `    -2`,
  where cycle 02's acceptance item 10 records MATLAB's `   -2.0000`. Cycle 02
  fixed the display half and left this value half to the LU rewrite, which
  decides the operation order. Settle it at this cycle's planning from a
  source, not by choosing an order that happens to give `-2.0000000000000004`;
  see the Known deviations row in `docs/ARCHITECTURE.md`
- **Complex results stay refusals until cycle 10**, which the project's
  owner decided on 2026-09-28 to build. An eigenvalue problem whose answer
  is complex, `eig([0 -1; 1 0])`, is a clean error beginning `Complex
  results are not supported.`, the prefix cycle 01d's refusals share, never
  a wrong real answer. Cycle 10 replaces the refusal with values
- **Every iterative method terminates** (invariant 6): the QR iteration,
  Jacobi sweeps and the one-sided Jacobi SVD each have an iteration cap and
  fail with a clean error past it, and a `NaN` or `Inf` input to `eig`,
  `svd`, `chol`, `qr`, `lu`, `rank`, `pinv` or `cond` is a clean error or a
  `NaN` result, never a hang. The texts are SplatCrab's own unless a
  source settles MATLAB's
- The singular-matrix warning goes through cycle 04's `Interp.err`, so
  under `--protocol` and `--ui` it lands in an `eval`'s `out`, in order,
  with nothing on stderr
- This cycle closes the Known bugs row for QA D26 (`inv` and `A^-1` of a
  singular matrix) and the Known deviations rows for square-only backslash
  and vectors-only `norm`; `sort` of a matrix stays with cycle 09

Two bullets that stood here, `matmul` swallowing `Inf` and `NaN` through its
sparsity shortcut and the absolute pivot tolerance in `solve`, were fixed by
cycle 01d and removed from this Scope in the same commit. The LU rewrite
inherits the rule rather than inventing a second one: `Matrix::singular_tol`
is `eps * n * max |A|` over the finite entries, and `solve` and `det` both
test a pivot against it with `<=`, which is what makes them agree on what
singular means. Keeping them in agreement is part of this cycle's job.

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.
- Complex eigenvalues and complex singular vectors, which need cycle 10.
- Sparse matrices.

## Design notes

### Files and types

- **`src/builtins/factor.rs`** (new) holds the numerics, with no `Interp`:
  `lu` returning an `Lu` (packed factors, `perm`, `sign`, and the `singular`
  and `zero_pivot` flags) with `det`, `solve`, `inverse` and `factors`;
  `qr`; `lstsq`; `chol`; `is_symmetric`, `eig_sym` and `eig_general`; `svd`
  with the `Vectors` choice (`None`, `Thin`, `Full`) and the `Svd` result;
  `zeros` and `eye`, which judge a computed shape through `check_shape`.
- **`src/value.rs`**: `Matrix::solve`, `inv` and `det` are thin wrappers over
  `factor::lu` and `factor::lstsq`. `solve` and `inv` now return
  `(Matrix, Option<String>)`, the warning beside the result; the old square-
  only elimination is gone. `singular_tol` is unchanged and `pub(crate)`.
- **`src/interp.rs`**: `Interp::warn` writes a returned warning through
  `emit_err`; `\`, `/` and `^` call it. `matrix_power` returns its warning
  too, and starts its product from the first factor rather than from `I`,
  because `I * X` is `NaN` wherever `X` is `Inf` (`0 * Inf`), and
  `[1 2; 2 4]^-1` has to be `Inf` everywhere as `inv` is.
- **`src/builtins/linalg.rs`**: `inv` warns; `norm` takes a matrix; fifteen
  new builtins, `lu qr chol eig svd rank pinv null orth cond kron cross triu
  tril magic` (the registry grows from 115 to 130).
- **`src/error.rs`**: `nonsquare_system`, `singular` and `norm_vectors_only`
  are gone; new are `singular_warning` and `rank_deficient_warning` (texts,
  not errors), `nonsquare_for`, `not_positive_definite`, `nonfinite_input`,
  `no_convergence`, `complex_eigenvalues`, `matrix_norm_type`,
  `cross_length`, `magic_order`, `tolerance_arg` and `ab_size_mismatch`
  (which `dot_size_mismatch` now calls, its text unchanged).

### Invariants

Column-major throughout: `factor.rs` reads and writes `Matrix` through
`get`/`set` and `data[c * rows + r]`. The one exception is internal:
`eig_general` works on a row-per-`Vec` copy because `orthes` and `hqr2` are
written that way, and converts back before returning. No index conversion
happens anywhere here; the one-based to zero-based boundary is untouched, and
so are the `end` stack and name resolution. Every result shape computed from
the operands goes through `check_shape` before it is allocated (`kron`, the
`n`-by-`k` answer of a system with no rows, `Q`, `U`, `V`, `P`, `S`), and all
output still leaves through `emit` and `emit_err`: `factor.rs` and `value.rs`
return a warning, they never write one, so under `--protocol` it lands in the
`eval`'s `out`, in order, and nothing reaches stderr (item 14).

### The shared LU and `det([1 2; 3 4])`

`factor::lu` keeps the arithmetic of the elimination `det` and `solve` had:
the multiplier is `a(i, k) / a(k, k)` (not a reciprocal times `a(i, k)`, as
LAPACK's `dgetf2` computes it), the update is `a(i, j) - l * a(k, j)`, and
`det` multiplies the pivots in order after the sign. So every finite value
the two gave before, they give again (a non-finite input can differ: the old
solve skipped a zero multiplier, so `[1 Inf; 0 1] \ [1; 1]` was `[-Inf; 1]`
and is now `NaN NaN`, because `1 - 0*Inf` is `NaN`), and `det([1 2; 3 4])` is still exactly `-2`;
`matrix_ops` and `demo_smoke` are unchanged. The verify-first bullet was not
settled from a source at planning, so this cycle did what the bullet
requires in that case: it chose no order to produce MATLAB's digits, and
nothing asserts the value beyond what the existing cases already did. The
Known deviations row stays, rescheduled to "later (verify first)".

Two details of the LU are new. The pivot is the *first* element of largest
magnitude, as BLAS `idamax` picks it (Rust's `max_by` had picked the last of
a tie; no existing case has a tie), and a `NaN` never displaces a number. A
column whose largest magnitude is exactly `0` is skipped, as `dgetf2` skips
it, and sets `zero_pivot`. The `singular` flag is the cycle-01d test, a pivot
`<=` `Matrix::singular_tol()`, and `det` returns exactly `0` when it is set, so
`det` is `0` precisely where `\` warns: `det_and_solve_agree_on_singular`
checks it on the old cases.

### Choices where the spec is silent

- **Which singular systems warn, and what they return.** A square system
  warns "Matrix is singular to working precision." whenever `singular` is set
  and returns what forward and back substitution give: `NaN NaN` for
  `[1 2; 2 4] \ [1; 2]` (a `0 / 0`), large values for a nearly singular one.
  `inv` warns on the same test, returns `Inf` everywhere when a pivot is
  exactly `0` (`inv([1 2; 2 4])`, `inv(0)`, item 11) and the computed inverse
  otherwise. `A^-n` is `inv` then powers. `lu` itself never warns.
- **Least squares.** A non-square `\` or `/` uses Householder QR with column
  pivoting. The rank is the number of leading `|R(i, i)|` above
  `max(m, n) * eps * |R(1, 1)|`, and the answer is the basic solution, zero in
  the non-pivot rows, which is the one MATLAB documents for a rank-deficient
  or underdetermined system. A rank below `min(m, n)` warns
  "Matrix is rank deficient to working precision (rank r).", SplatCrab's own
  text; MATLAB is understood to name a tolerance as well, unverified. An `A`
  with a `NaN` or `Inf` gives an all-`NaN` answer with no warning. The
  square path is never replaced by QR, even when it is singular.
- **`qr`** is the full factorisation (`Q` is `m`-by-`m`), `R = qr(A)` is `R`
  alone, and there is no economy or pivoted form. The reflector is LAPACK's
  (`beta = -sign(x(1)) * norm(x)`, identity when nothing is below the
  diagonal), so `R`'s diagonal may be negative; item 3 takes `abs`.
- **`chol`** reads the diagonal and upper triangle only, as MATLAB's does,
  and does not check symmetry. A pivot that is not `> 0` fails, so a `NaN`
  diagonal is "Matrix must be positive definite." (item 13's `chol` case:
  **exit 1**). `[R, p] = chol(A)` gives the failing column and the leading
  block instead of the error. A non-square matrix is "Matrix must be square
  for 'chol'.".
- **`eig`.** "Symmetric" is exact equality `A(i, j) == A(j, i)`. The
  symmetric path returns ascending eigenvalues and orthonormal vectors; the
  general path returns them in the order the QR iteration deflates them,
  with each vector scaled to unit 2-norm, and MATLAB's order is not claimed.
  A complex pair is found by the 2-by-2 block test of `hqr2` (`q < 0`) and
  is the refusal "Complex results are not supported. The eigenvalues of this
  matrix are complex." (item 12). A `NaN` or `Inf` is refused before any
  iteration with "Input to 'eig' must not contain NaN or Inf." (item 13's
  `eig` case: **exit 1**). `eig([])` is 0x1. A non-square matrix is
  "Matrix must be square for 'eig'.".
- **The general eigensolver** adds one fix to JAMA's `hqr2`: an exactly zero
  sub-diagonal element always deflates. JAMA's test `|h| < eps * s` never
  passes for the zero matrix, whose norm and so `s` are `0`, and it iterated
  to the cap.
- **`svd`** is one-sided Jacobi on `A / max|A|` (on `A'` for a wide `A`), a
  pair rotated while `|gamma| > m * eps * sqrt(alpha * beta)`; `m * eps`
  rather than `eps` so that the roundoff in `gamma` itself cannot keep a
  converged pair rotating. `U` is completed to `m`-by-`m` from the `Q` of a
  Householder QR of its leading columns. `pinv` and `orth` use the thin
  vectors, so `pinv` of a 100000-element column needs no 100000-by-100000
  `U`. A `NaN` or `Inf` is refused as for `eig` (item 13's `svd` case:
  **exit 1**), and so it is by every builtin built on the SVD, in its own
  name: `rank`, `pinv`, `null`, `orth`, `cond`.
- **Tolerances.** `rank` counts singular values above
  `max(m, n) * eps(s(1))`, the MATLAB `rank` page's default; `null` and
  `orth` use the same tolerance so that `rank(A) + size(null(A), 2)` is
  `size(A, 2)`. `pinv` drops values at or below `max(m, n) * s(1) * eps`,
  the `pinv` page's default. `rank(A, tol)` and `pinv(A, tol)` take one.
- **`cond(A)`** is `s(1) / s(end)`, `Inf` when the smallest is `0` (the zero
  matrix included) and `0` for an empty matrix; there is no `cond(A, p)`.
- **Matrix `norm`.** `1`, `2`, `Inf` (or `'inf'`) and `'fro'`; any other
  order of a matrix is "Matrix norm type for 'norm' must be 1, 2, Inf or
  'fro'.". A `NaN` anywhere is `NaN` for all four; an `Inf` makes the 2-norm
  `Inf` without an SVD, which would refuse it. `'fro'` of a vector is still
  its 2-norm. A vector's norms are cycle 01c's, unchanged.
- **`cross`** needs two arrays of one size and works along the first
  dimension of length 3 (a 3-element row or column, or column by column of a
  3-by-n); otherwise "A and B must be the same size for 'cross'." or "A and B
  must have a dimension of length 3 for 'cross'.". A row crossed with a
  column is refused rather than guessed.
- **`triu` and `tril`** keep the class, as the rearrangements do, and take
  their `k` as `diag` does, refusing a non-integer with `diag`'s message.
- **`magic(n)`** floors `n`, returns `[]` below 1, refuses `NaN` or a char
  with "Order for 'magic' must be a real scalar.", and follows MATLAB's own
  construction (odd, doubly even, singly even); the unit test checks every
  order to 12 for the magic property and `magic(3)` and `magic(4)` for
  layout.
- **NaN and Inf, per builtin** (the Scope's "record each choice"): `eig`,
  `svd`, `rank`, `pinv`, `null`, `orth` and `cond` are the clean error
  `Input to '<name>' must not contain NaN or Inf.`; `chol` is the
  positive-definite error for a `NaN` pivot and computes through an `Inf`;
  `lu`, `qr`, `det`, `inv`, `\` and `/` have no iteration and let the value
  spread to a `NaN` (or `Inf`) result; matrix `norm` is `NaN`, or `Inf`.

### Iteration caps

`JACOBI_SWEEPS` and `SVD_SWEEPS` are 100 sweeps, where convergence is
quadratic and a handful is usual; the QR iteration allows
`qr_iterations(n) = 30 * max(10, n)` iterations per eigenvalue, LAPACK
`dlahqr`'s budget, reset at each deflation, so the total is bounded by `n`
times that. Past a cap the answer is "'eig' did not converge within its
iteration limit." (or `'svd'`, also for `rank`, `pinv`, `null`, `orth`,
`cond` and the matrix 2-norm, which reach it through `svd`). The caps are
parameters of `factor.rs`, and its unit tests call each method with a cap of
`0` to reach the error, since no input known reaches one: with non-finite
input refused and the matrix scaled by its largest magnitude, the iterations
have converged on every input tried, rank-deficient and zero ones included.
There is no golden case for `no_convergence` for that reason.

### Bytes settled in testing

Each line below is a byte of a golden case that the Acceptance tests do not
fix outright, settled from this spec's rules and the choices above.

- **Item 8**: `14.9330`, not planning's `14.9331` (the item is corrected).
- **Item 10**: `singular_backslash_warns` shows the result by its size,
  `     2     1`, since the item fixes the warning and the exit code only;
  `00-baseline/err_singular` pins the `NaN NaN` recorded above.
- **Item 11**: `inv(0)` displays `x =`, a blank line and `   Inf`, cycle
  02's non-finite column rule; `[1 2; 2 4]^-1` is `inv`'s `Inf` matrix,
  as the item's "behaves the same" and the `matrix_power` note require.
- **Item 13**: all three are clean errors, **exit 1**, with an empty `.out`
  and the texts recorded above: `Input to 'eig' must not contain NaN or
  Inf.`, `Input to 'svd' must not contain NaN or Inf.` (SplatCrab's own) and
  `Matrix must be positive definite.` (the spec's). None is a `NaN` result,
  so each keeps its `err_` name.
- **Item 14**: a warning line inside `out` is the `Warning: ` line with its
  `\n`, in the place `emit_err` wrote it, as cycle 04's
  `warning_in_protocol_out` has it.
- **`/` least squares**: `slash_least_squares` is item 1 transposed,
  `0.6667 0.5000` and a size of `     1     2`.
- **The rank-deficiency warning**: `lsq_rank_deficient_warns` pins
  `Warning: Matrix is rank deficient to working precision (rank 1).` with
  exit 0. Which of two equal-norm columns the pivoted QR keeps is a tie, so
  the case checks the basic solution by its properties (one exact zero, a
  sum of `2`) rather than by its entries.
- **SplatCrab's own error texts**, each pinned in its `err_*` case, exit 1:
  `Matrix must be square for 'eig'.` and `... for 'chol'.`, `Matrix norm
  type for 'norm' must be 1, 2, Inf or 'fro'.`, `A and B must be the same
  size for 'cross'.`, `A and B must have a dimension of length 3 for
  'cross'.`, `Order for 'magic' must be a real scalar.`, and `Tolerance for
  'rank' must be a real scalar.` and the same for `'pinv'`. `'eig' did not
  converge within its iteration limit.` and the `'svd'` form stay with unit
  tests only (Iteration caps).
- **The SVD floor.** A crash probe of `svd([1 2 3; 4 5 7; 0 0 0])` reached
  the sweep cap: a column cancelled by the rotations leaves residue in the
  span of the others, which no rotation can make orthogonal to them, so its
  squared norm shrank to an underflow and the relative test never passed.
  `jacobi_tall` now sets a column whose norm has fallen to
  `eps^2 * ||A||_F` to exactly zero, which moves `A * V - U * S` by less than
  that; `svd_of_a_matrix_with_a_zero_row_converges` is the unit test and
  `svd_zero_row_converges` the golden case.

### Deviations accepted

- `det([1 2; 3 4])` is exactly `-2` (above); the row stays in Known
  deviations.
- A system singular only to working precision warns with the
  exactly-singular text; MATLAB is understood to distinguish "close to
  singular or badly scaled" with an `RCOND`. `det` is exactly `0` wherever
  `\` warns, where MATLAB's is the product of the pivots.
- `eig`'s refusal of `NaN` and `Inf` and its order for a non-symmetric
  matrix, `svd`'s refusal, the rank-deficiency text and every other text above
  are SplatCrab's own except "Matrix is singular to working precision." and
  "Matrix must be positive definite.", which the spec records.
- `qr`, `svd` and `eig` have no economy, pivoted or balancing options, and
  `cond` no second argument.
- The SVD floor (above) sets a column whose norm falls to `eps^2 * ||A||_F`
  to zero, so a singular value that small is lost: `svd([1 1e-40; 0 1e-40])`
  gives `0` for the second and `cond` `Inf`, where the true values are about
  `1e-40` and `1.4e40`. The result is still within `eps * ||A||` and `rank`
  is unchanged; only a condition number above about `1e31` is affected.
  Found at review.
- Least squares overflows where the answer does not: `(1.7e308 * [1 1; 1 2;
  1 3]) \ [1; 2; 2]` is `NaN NaN` with no warning, because `R(1, 1)`
  overflows. At `1e300` the same system is right. Found in testing.
- Two golden cases outside this module assert the old behaviour and change
  with it: `00-baseline/err_singular` and
  `01d-numerics-and-printf/solve_relative_pivot` each expect a singular
  `\` to exit 1, where it now warns and exits 0 (QA D26's backslash half, a
  Scope bullet of this cycle). Each gains an `.exit` of `0`, and its `.err`,
  `Matrix is singular to working precision.`, is unchanged and now matches
  the warning. `solve_relative_pivot` suppresses its result, so its `.out`
  is unchanged; `err_singular` displays it, so its empty `.out` becomes
  `ans =` over `NaN NaN`, the result recorded above for exactly this system
  (`2 - 0.5 * 4` and `1 - 0.5 * 2` are exact zeros, so `0 / 0` is not
  roundoff-sensitive). The comments of `solve_relative_pivot.m` that said
  the singular solve errors are rewritten; `err_singular` keeps its name.

**Fixed at review.** Two findings, neither a wrong answer:

- The Hessenberg reduction, the shifted QR iteration and the non-symmetric
  eigenvector back-substitution had unit tests but no golden case: the one
  non-symmetric case was a 2x2, which `hqr2` solves directly.
  `eig_nonsymmetric_hessenberg` runs a 4x4 whose spectrum is 1 2 3 4 by
  construction, `A = T * diag([1 2 3 4]) / T`, and checks its vectors by
  their residual.
- A matrix `norm` of an order the vector rules also refuse, `norm(A, 0)`,
  `norm(A, -1)` or `norm(A, 'bogus')`, gave the vector text, which offers
  `-Inf` and any positive real. It now gives the matrix text these notes
  record (`err_norm_matrix_order`).

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/08-linear-algebra/`. Expected output is written by hand from MATLAB
semantics wherever feasible; the `.err` substring is always chosen by hand.
Every new error message needs an `err_*` case.

1. `A = [1 1; 1 2; 1 3]; b = [1; 2; 2]; x = A \ b; fprintf('%.4f %.4f\n', x)` → `0.6667 0.5000` Cases: backslash_least_squares, slash_least_squares, lsq_rank_deficient_warns.
2. `A = [1 2; 3 4]; [L, U, P] = lu(A); fprintf('%.4f\n', L(2, 1)); disp(norm(P * A - L * U) < 1e-12)` → `0.3333\n   1` (a comparison is a logical, four wide since cycle 02) Cases: lu_three_outputs.
3. `[Q, R] = qr([1 2; 3 4]); fprintf('%.4f\n', abs(R(1, 1))); fprintf('%.4f\n', norm(Q * R - [1 2; 3 4]))` → `3.1623\n0.0000` Cases: qr_factors.
4. `R = chol([4 2; 2 3]); fprintf('%.4f ', R); fprintf('\n'); chol([1 2; 2 1])` → `2.0000 0.0000 1.0000 1.4142 ` then err `Matrix must be positive definite.` Cases: chol_upper_factor, err_chol_not_positive_definite, err_chol_nonsquare.
5. `fprintf('%.4f %.4f\n', sort(eig([4 1; 2 3]))); e = eig([2 1; 1 2]); fprintf('%.4f %.4f\n', e); [V, D] = eig([2 0; 0 3]); fprintf('%.4f %.4f\n', diag(D))` → `2.0000 5.0000\n1.0000 3.0000\n2.0000 3.0000`. Rewritten at planning with `%.4f`: an iteration's eigenvalues need not be exact integers, and `disp` of `2.0000000000000004` is `2.0000`, not `2` (the roundoff rule in `docs/TESTING.md`) Cases: eig_nonsymmetric_real, eig_symmetric_ascending, eig_two_outputs, eig_vectors_residual, err_eig_nonsquare.
6. `fprintf('%.4f %.4f\n', svd([3 0; 0 4])); [U, S, V] = svd([1 2; 3 4]); fprintf('%.4f\n', norm(U * S * V' - [1 2; 3 4]))` → `4.0000 3.0000\n0.0000` (the first line rewritten with `%.4f` at planning, as item 5's) Cases: svd_values_descending, svd_three_outputs, svd_zero_row_converges.
7. `disp(rank([1 2; 2 4])); disp(rank(eye(3))); fprintf('%.4f ', pinv([1 2; 2 4])); fprintf('\n')` → `     1\n     3\n0.0400 0.0800 0.0800 0.1600 ` Cases: rank_deficient_and_full, pinv_rank_deficient, null_basis, orth_basis, err_rank_tolerance, err_pinv_tolerance.
8. `A = [1 2; 3 4]; fprintf('%.4f %.4f %.4f %.4f %.4f\n', norm(A), norm(A, 'fro'), norm(A, 1), norm(A, inf), cond(A))` → `5.4650 5.4772 6.0000 7.0000 14.9330`. The vector p-norms, such as `norm([3 4], 1)`, landed in cycle 01c and are not claimed here. Corrected in testing from planning's `14.9331`, an arithmetic slip: `cond(A)` is `(30 + sqrt(884))/4 = 14.933034...`, well clear of the `14.93305` tie, so `%.4f` prints `14.9330` Cases: norm_matrix_and_cond, err_norm_matrix_type.
9. `disp(kron([1 2], [1; 1])); disp(cross([1 0 0], [0 1 0])); disp(magic(3))` → `     1     2\n     1     2\n     0     0     1\n     8     1     6\n     3     5     7\n     4     9     2` Cases: kron_two_vectors, cross_unit_vectors, magic_three, triu_tril, err_cross_size_mismatch, err_cross_no_length_three, err_magic_order.
10. `x = [1 2; 2 4] \ [1; 2]` → stderr `Warning: Matrix is singular to working precision.` with a result on stdout and exit 0; regression `disp([1 2; 3 4] \ [5; 6])` → `   -4.0000\n    4.5000` Cases: singular_backslash_warns, backslash_square_regression.
11. `x = inv([1 2; 2 4])` → stderr `Warning: Matrix is singular to working precision.`, then `x =\n\n   Inf   Inf\n   Inf   Inf\n` on stdout, exit 0; `y = [1 2; 2 4]^-1` behaves the same Cases: inv_singular_warns_inf, mpower_singular_warns_inf, inv_zero_warns_inf.
12. `eig([0 -1; 1 0])` → err containing `Complex results are not supported.`, exit 1 Cases: err_eig_complex_eigenvalues.
13. `eig([NaN 1; 1 1])`, `svd([Inf 0; 0 1])` and `chol([NaN 0; 0 1])` each end with exit 0 or 1, never 101, 134 or a hang, and the case asserts the exact code the implementation chose and recorded; any text asserted is SplatCrab's own, pinned in its `err_*` case Cases: err_eig_nan_input, err_svd_inf_input, err_chol_nan_input.
14. Under `--protocol`, `{"id":1,"op":"eval","code":"x = inv([1 2; 2 4]);"}` → an `out` holding the warning line `Warning: Matrix is singular to working precision.` and nothing on stderr Cases: singular_warning_protocol_out, singular_warning_protocol_in_order.

## Status

Done (2026-09-28)
