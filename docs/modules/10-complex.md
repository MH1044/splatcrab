# 10 — Complex

## Goal

`Matrix.im: Option<Vec<f64>>`, `1i` literals, complex arithmetic and display, `real imag conj angle isreal`, conjugate transpose, complex `roots`/`eig`, `fft ifft`

After this module a user can do things that were not possible before; every
bullet in Scope must be demonstrable by at least one acceptance test below.

## Scope

- `Matrix.im: Option<Vec<f64>>`
- `1i` literals
- complex arithmetic and display
- `real imag conj angle isreal`
- conjugate transpose
- complex `roots`/`eig`
- `fft ifft`

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.

- Whether to build this module was an open question until 2026-09-28,
  when the project's owner decided that it is built. Until it lands, every
  builtin that would return a complex result raises a clear error: cycle
  01d put that error in place for the nine inputs QA D14 named, through
  `math::Real`, `math::powf_real` and the three `error::complex_*`
  constructors, and cycles 08 and 09 route their own complex results
  through the same refusals. This cycle's job is to replace those refusals
  with values, not to add any.

## Design notes

Filled in as decisions are made. Start from the
"Key designs to preserve" section of docs/ARCHITECTURE.md and record here:
which files and types changed, which invariants were preserved (column-major
storage, 1-based to 0-based conversion at the index boundary, the `end` stack,
name resolution order, the output sink), and any deviation from MATLAB that
was accepted deliberately.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/10-complex/`. Expected output is written by hand from MATLAB
semantics wherever feasible; the `.err` substring is always chosen by hand.
Every new error message needs an `err_*` case.

1. `z = 3 + 4i; disp(abs(z)); disp(real(z)); disp(imag(z)); disp(conj(z))` → `     5\n     3\n     4\n   3.0000 - 4.0000i`
2. `z = sqrt(-4)` → `z =\n\n   0.0000 + 2.0000i\n`
3. `r = roots([1 0 1]); fprintf('%.4f %.4f\n', imag(r))` → `1.0000 -1.0000`
4. `e = eig([0 -1; 1 0]); disp(isreal(e)); fprintf('%.4f\n', abs(e))` → `     0\n1.0000\n1.0000`
5. `disp(fft([1 0 0 0])); y = fft([1 2 3 4]); fprintf('%.4f ', abs(y)); fprintf('\n'); disp(real(ifft(fft([1 2 3]))))` → `     1     1     1     1\n10.0000 2.8284 2.0000 2.8284 \n     1     2     3`
6. `disp([1+2i 3]')` → `   1.0000 - 2.0000i\n   3.0000 + 0.0000i`; `disp([1+2i 3].')` → `   1.0000 + 2.0000i\n   3.0000 + 0.0000i`
7. `disp(class(1i)); disp(isreal(1i * 0))` → `double\n     0` (lock the chosen complex-flag semantics in the spec).

## Status

Planned
