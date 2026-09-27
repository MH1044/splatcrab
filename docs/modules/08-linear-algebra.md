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

Filled in as decisions are made. Start from the
"Key designs to preserve" section of docs/ARCHITECTURE.md and record here:
which files and types changed, which invariants were preserved (column-major
storage, 1-based to 0-based conversion at the index boundary, the `end` stack,
name resolution order, the output sink), and any deviation from MATLAB that
was accepted deliberately.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/08-linear-algebra/`. Expected output is written by hand from MATLAB
semantics wherever feasible; the `.err` substring is always chosen by hand.
Every new error message needs an `err_*` case.

1. `A = [1 1; 1 2; 1 3]; b = [1; 2; 2]; x = A \ b; fprintf('%.4f %.4f\n', x)` → `0.6667 0.5000`
2. `A = [1 2; 3 4]; [L, U, P] = lu(A); fprintf('%.4f\n', L(2, 1)); disp(norm(P * A - L * U) < 1e-12)` → `0.3333\n     1`
3. `[Q, R] = qr([1 2; 3 4]); fprintf('%.4f\n', abs(R(1, 1))); fprintf('%.4f\n', norm(Q * R - [1 2; 3 4]))` → `3.1623\n0.0000`
4. `R = chol([4 2; 2 3]); fprintf('%.4f ', R); fprintf('\n'); chol([1 2; 2 1])` → `2.0000 0.0000 1.0000 1.4142 ` then err `Matrix must be positive definite.`
5. `disp(sort(eig([4 1; 2 3]))'); e = eig([2 1; 1 2]); fprintf('%.4f %.4f\n', e); [V, D] = eig([2 0; 0 3]); disp(diag(D)')` → `     2     5\n1.0000 3.0000\n     2     3`
6. `disp(svd([3 0; 0 4])'); [U, S, V] = svd([1 2; 3 4]); fprintf('%.4f\n', norm(U * S * V' - [1 2; 3 4]))` → `     4     3\n0.0000`
7. `disp(rank([1 2; 2 4])); disp(rank(eye(3))); fprintf('%.4f ', pinv([1 2; 2 4])); fprintf('\n')` → `     1\n     3\n0.0400 0.0800 0.0800 0.1600 `
8. `A = [1 2; 3 4]; fprintf('%.4f %.4f %.4f %.4f %.4f\n', norm(A), norm(A, 'fro'), norm(A, 1), norm(A, inf), cond(A))` → `5.4650 5.4772 6.0000 7.0000 14.9331`. The vector p-norms, such as `norm([3 4], 1)`, landed in cycle 01c and are not claimed here
9. `disp(kron([1 2], [1; 1])); disp(cross([1 0 0], [0 1 0])); disp(magic(3))` → `     1     2\n     1     2\n     0     0     1\n     8     1     6\n     3     5     7\n     4     9     2`
10. `x = [1 2; 2 4] \ [1; 2]` → stderr `Warning: Matrix is singular to working precision.` with a result on stdout and exit 0; regression `disp([1 2; 3 4] \ [5; 6])` → `   -4.0000\n    4.5000`
11. `x = inv([1 2; 2 4])` → stderr `Warning: Matrix is singular to working precision.`, then `x =\n\n   Inf   Inf\n   Inf   Inf\n` on stdout, exit 0; `y = [1 2; 2 4]^-1` behaves the same

## Status

Planned
