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
- `fft ifft`, `O(n log n)` for every length (a power of two directly,
  any other through Bluestein's algorithm or a mixed radix; record which),
  so no length makes them quadratic
- **The complex flag, from MathWorks' own pages** (the `complex` and
  `isreal` pages, fetched at planning): `complex(a, b)` makes complex
  storage even when `b` is all zeros, so `isreal(complex(1, 0))` is false;
  "the addition `a + 0i` returns a strictly real result", and an arithmetic
  result whose imaginary parts are all zero is stored real, as the sum of
  `3+4i` and `5-4i` is. So every arithmetic operator and element-wise
  function drops an all-zero imaginary part from its result, and `complex`
  is the one way to keep one. `complex` joins the builtins
- **Comparisons, from the MathWorks relational-operators page**: "The
  operators >, <, >=, and <= use only the real part of the operands";
  `==` and `~=` test both parts
- **Formatted output, from the MathWorks `sprintf` page**: "Numeric
  conversions print only the real component of complex numbers", in
  `fprintf` and `sprintf` alike
- **The refusals this cycle replaces**, every one that begins `Complex
  results are not supported.`: cycle 01d's nine (`sqrt`, `log`, `log2`,
  `log10`, `asin`, `acos`, the `^` and `.^` operators and `power` of a
  negative base to a fractional power), cycle 08's complex `eig` and cycle
  09's complex `roots`. Each of their eleven `err_*` cases, in `01d`, `08`
  and `09`, is deleted in this cycle's commit and replaced by a value case
  for the same input in this cycle's directory. After this cycle the
  prefix, and the three `error::complex_*` constructors, are gone
- **A kernel that ignores `im` refuses a complex input**, never drops the
  imaginary part in silence: a complex argument to a builtin this cycle
  does not teach complex numbers (`sort`, `max`, `min`, `floor`, `mod`,
  the solvers of cycle 09, and every other) is a clean error with
  SplatCrab's own text. The operators, `sum`, `prod`, `mean`, `cumsum`,
  `abs`, `angle`, `exp`, `sin` and `cos`, and every function whose refusal
  this cycle replaces (`sqrt`, `log`, `log2`, `log10`, `asin`, `acos`,
  `power`), take complex input as well as returning it
- **The lexer**: `1i`, `2.5j`, `1e3i`; `i` and `j` are the imaginary unit
  unless a variable of that name exists, invariant 4's order
- Complex values in cells and structs, and the protocol's `workspace`
  lists one with class `double`; U0's `vars` shape is unchanged

Planning added the bullets after `fft ifft`: the four behaviours taken
from MathWorks pages rather than recalled, the list of refusals and their
cases, the rule against silently dropping an imaginary part, and the lexer
and container details.

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

### Files and types

- **`src/value.rs`**: `Matrix` gains `im: Option<Vec<f64>>`, the imaginary
  parts, column-major like `data`, `None` for real storage. A complex value
  is class `double`; no logical or char is ever complex (`to_class` refuses
  to make one, `Complex values cannot be converted to logicals.` or the char
  form of the refusal below). New on `Matrix`: `is_complex`, `c(k)` (element
  `k` as a complex scalar), `with_im` and `normalized` (the flag rule),
  `from_c`, `complex_parts` (the one constructor that keeps an all-zero
  imaginary part), `conj`, `ctranspose`, `real_part`, `imag_part`,
  `element`, `map_c`, `zip_c` (the complex `zip`, with the same broadcasting
  and the same `check_shape`), `require_real` (the refusal). `transpose`
  carries `im`; `matmul`, `solve` and `inv` take complex operands; the
  display has a complex branch, `complex_cells`.
- **`src/builtins/complex.rs`** (new): `C`, the complex scalar, with its
  arithmetic and `abs arg exp ln log2 log10 sqrt sin cos asin acos powi
  pow`; the builtins `i j real imag conj angle isreal complex fft ifft`,
  ten, so the registry holds 173; the transforms `dft`, `radix2` and
  `bluestein`.
- **`src/builtins/mod.rs`**: `TAKES_COMPLEX`, the builtins that take a
  complex argument, and `complex_gate`, which `Interp::call_builtin` runs
  before every builtin.
- **`src/builtins/math.rs`**: `sqrt exp log log2 log10 sin cos asin acos`
  go through `complex_unary` over `C`, `abs` takes a modulus, `power` is
  `zip_c` of `C::pow`, and `sum prod mean cumsum` have complex branches
  (`complex_reduction`, `reduce_c`). `Real`, `real_unary`, `real_binary` and
  `powf_real` are gone.
- **`src/builtins/factor.rs`**: `eig_general` returns `(Vec<C>, Matrix)`;
  `hqr2` keeps JAMA's `e` (the imaginary parts) and does JAMA's complex
  back-substitution, so a complex pair gives complex values and vectors.
  `linalg::eig` and `numerics::roots` pass them on.
- **`src/lexer.rs`**: `Token::Imag` and `Token::DotTranspose`.
  **`src/parser.rs`**: `Expr::Imag` and `Expr::DotTranspose`; `render` gives
  `2i` and `x.'` back for `func2str`.
- **`src/interp.rs`**: `complex_binary` for an operator with a complex
  operand; `.^` and scalar `^` of real operands go through `C::pow` too,
  since they can be complex. `gather`, `scatter` (through
  `assign_matrix`), deletion, `hcat`, `vcat`, the `for` loop, `switch` and
  `Value::element` carry `im`.
- **`src/error.rs`**: the five `complex_*` refusals of cycles 01d, 08 and
  09 are gone, and with them the `Complex results are not supported.`
  prefix. Two messages are new, both SplatCrab's own:
  `complex_argument(name)`, `Complex values are not supported by 'sort'.`,
  and `complex_to_logical()`, `Complex values cannot be converted to
  logicals.`

### The flag rule, and where it applies

Every operation stores its result by the rule the spec takes from
MathWorks: real when every imaginary part is zero, through
`Matrix::with_im`. `complex(a, b)` is the only exception. The spec left
indexing, concatenation and assignment to this cycle; they follow the same
rule, so there is one rule to learn:

- **Indexing**: `z = [1+2i 3]; z(2)` is the real `3`, and `z(1)` complex.
- **Concatenation**: complex if any operand is, the others' imaginary parts
  zero, then the rule: `[complex(1, 0) 2]` is real.
- **Assignment**: the analogue of the class rule: a double target assigned
  a complex value becomes complex (`x = [1 2]; x(2) = 3i`); a logical or a
  char target refuses one, since neither class converts a complex value;
  the result is then stored by the rule, so `z = [1i 2]; z(1) = 5` is real.
  Deletion and the transposes follow the rule too.
- **Copies**: a value copied whole keeps its storage: `y = x`, an argument,
  a return value, an element of a cell and a field of a struct. So
  `c = {complex(1, 0)}; isreal(c{1})` is false, while `x = complex(1, 0);
  isreal(x(1))` is true.

A zero imaginary part is zero whatever its sign: the rule, the display and
the branch cuts all read `-0` as `+0`.

### A kernel that ignores `im` refuses a complex input

This is the failure the cycle must never allow, so it is closed centrally
rather than builtin by builtin:

- **Builtins.** `complex_gate` refuses a complex matrix argument to every
  builtin not on `TAKES_COMPLEX`, before it runs, so the other 140-odd
  builtins keep reading `data` as the real array it is. On the list: the
  functions of complex numbers and the transforms; the kernels this cycle
  taught (`abs sum prod mean cumsum exp sin cos sqrt log log2 log10 asin
  acos power`); `disp`, `fprintf` and `sprintf`; the shape and class
  queries, which read no element; the functions that pass a value through
  whole (`struct getfield setfield deal feval arrayfun cellfun num2cell
  cell2mat`); `isequal` (both parts, so `isequal(complex(1, 0), 1)` is
  true); `double`; and `transpose`. `complex` itself is not on it, so
  `complex(1i, 2)` is the gate's refusal, and neither are `eig` and
  `roots`, which give complex results of real input but refuse complex
  input.
- **Inside those builtins.** `args::scalar` and `args::size_list` refuse a
  complex dimension, size or option (`sum(z, 2i)`); the uniform outputs of
  `cellfun` and `arrayfun` collect the imaginary parts; `cell2mat`
  concatenates as a bracket does; `any` and `all` refuse.
- **Solvers.** A complex value a solver's function returns is refused in
  the solver's name (`fzero(@(x) x + 1i, 0)`); a complex starting point is
  the gate's refusal.
- **The interpreter's own reads.** `if`, `while`, `&&`, `||`, `&`, `|` and
  `~` refuse a complex value (`truth`, `logical_scalar`, `complex_binary`);
  a complex subscript is the ordinary invalid-index error; a complex
  operand of `:` is `Complex values are not supported by ':'.`

### Arithmetic

`C` multiplies componentwise when a factor's imaginary part is zero and
divides componentwise when the divisor's is, so a real factor never makes a
`NaN` from `0 * Inf` in a part it does not touch, and `(1 + 2i) / 0` is
`Inf + Infi`; any other quotient is Smith's scaled division. The matrix
product is real products of the parts (two when one side is real, four
otherwise). A complex `A \ B`, `B / A` and `A^-n` go through the real
embedding `[Ar -Ai; Ai Ar]`, which reuses `factor::lu` and `factor::lstsq`
unchanged: it is singular exactly when `A` is, so the singular warning is
the same, and its rank is twice `A`'s, so the rank-deficiency warning
names half of it, rounded up. A complex exponent of a matrix, or any
non-integer one, is `Only integer matrix powers are supported.`

**Powers.** `C::pow` is the real `powf` wherever that is real (a
non-negative base, an integer, infinite or `NaN` exponent), so every real
result is unchanged; an integer power of a complex base is repeated
squaring, so `(1i)^2` is exactly `-1` and real; anything else is
`exp(p * log(z))` on the principal branch, so `(-8)^(1/3)` is
`1 + 1.7321i` through `^`, `.^` and `power` alike.

**Branch cuts.** `sqrt`, `log`, `log2`, `log10` and a non-integer power
cut along the negative real axis; `asin` and `acos` along the real axis
outside `[-1, 1]`. A point on a cut is taken from above: `sqrt(-4)` is
`2i`, `log(-1)` is `pi*i`, `asin(2)` is `1.5708 - 1.3170i`, `acos(2)` is
`1.3170i`, `acos(-2)` is `3.1416 - 1.3170i`. `asin` and `acos` are the
logarithmic formulas `-i*log(iz + sqrt(1 - z^2))` and
`-i*log(z + i*sqrt(1 - z^2))`. Each function is exactly its real
counterpart on its real domain, and `log2` and `log10` use `f64::log2` and
`f64::log10` there, so `log2(8)` is exactly `3`. On the imaginary axis
`sin(yi)` is exactly `sinh(y) i` and `cos(yi)` the real `cosh(y)`, so an
overflow gives `Inf` rather than a `NaN` from `0 * Inf` (in testing, found by
probing `sin(1000i)`; case `sin_cos_imaginary_axis`).

Operations on non-finite values follow the componentwise rule above and
IEEE arithmetic, with no special cases beyond it: `Inf * 1i` is
`NaN + Infi`, since the real factor meets the zero real part; `1i / 0` is
`NaN + Infi`; `1e308i * 10` is `0 + Infi`; `NaN + 1i` displays
`   NaN + 1.0000i`.

### `eig` and `roots`

`hqr2` records the pair its 2x2 block gives, `x + p +- z*i`, as JAMA does
(`e = [z, -z]`), instead of refusing it, and the back-substitution solves
the real and complex 2x2 systems the quasi-triangular form holds (JAMA's
`cdiv` is `C`'s division). A pair's vectors are `V(:, k) + V(:, k+1)*i`
and its conjugate, each scaled to unit 2-norm; the value with the positive
imaginary part comes first, so `eig([0 -1; 1 0])` is `0 + 1i; 0 - 1i`, as
is `roots([1 0 1])`. The phase of a complex eigenvector is whatever the
back-substitution gives; MATLAB's is not documented. Unit tests check
`A*V = V*D` over complex scalars for pairs, a 3x3 rotation, random 6x6
matrices and a companion matrix. A real root of multiplicity three or more
now comes back as a complex pair a rounding error apart, as MATLAB's does,
rather than being refused.

### `fft` and `ifft`

Of a row along the row, and of any other array down each column, the same
shape as the argument. A power-of-two length is the iterative radix-2
Cooley-Tukey transform; every other length is Bluestein's algorithm, the
transform as a convolution with the chirp `exp(-pi*i*k^2/n)`, done by
radix-2 transforms of the next power of two at or above `2n - 1`. So every
length is O(n log n); a unit test and a golden case run a prime length of
100,003. The twiddle angles are reduced in integers first and the quarter
turns are exact, which is why `fft([1 0 0 0])` is exactly real and ones,
and stored real. `ifft` scales by `1/n`.

### Display

A complex array is written element by element as the real part, the sign of
the imaginary part and its magnitude with an `i`: `   3.0000 - 4.0000i`.
Both parts always have four decimals, a zero `0.0000` (the spec's
`   0.0000 + 2.0000i`), and `-0` is written as `0`, so `[1+2i 3]'` ends in
`3.0000 + 0.0000i`. The real field is three wider than the widest real part
without its sign, so a minus takes one of the three spaces before it; the
imaginary field is as wide as the widest magnitude; every column has the
same width and columns are simply side by side. The layout is chosen from
the largest finite magnitude among both parts, by the real display's rule:
from `0.01` up to below `1000` as they are, and outside it a scalar in
short `e` format for both parts (`   1.0000e+05 + 2.0000e+00i`) and an
array under the `   1.0e+05 *` scale factor shared by both. A matrix
wider than 80 characters wraps into `Columns 1 through 4` blocks as a real
one does, each column being about twenty characters wide. A struct field
shows a complex scalar as its display does and a complex row as
`[1×2 double]`; a cell shows `{[1.0000 + 2.0000i]}`. None of this layout
was checked against a MATLAB run beyond the spec's recorded forms.

### The lexer

A number with `i` or `j` straight after it, not followed by more of a name,
is `Token::Imag`: `1i`, `2.5j`, `1e3i`, `.5i`; `2ix` is `2` and `ix`.
`i` and `j` are builtins, so invariant 4 puts a variable of the name first
and `clear i` brings the unit back; the lexer's command-syntax rule sees
them as it sees any name. `.'` became `Token::DotTranspose`.

### Invariants

Column-major storage holds for `im` as for `data`, and every new result
shape goes through `check_shape` (`zip_c`, `reduce_c`, the matrix product's
parts). Subscripts become zero-based only in `eval_index_args`, which now
refuses a complex one. The `end` stack, name resolution order (the unit is
two builtins), and the output sink are untouched; no new iteration without
a cap: the transforms have a fixed trip count and the QR iteration keeps
its cap.

### Bytes settled in testing

- **The refusal texts** are SplatCrab's own, since no MathWorks text was
  taken: `Complex values are not supported by '<name>'.`, pinned by the
  `err_*` cases of item 12 for `sort max min floor mod fzero`, by
  `err_colon_complex_operand` for `':'` and by
  `err_char_target_complex_assign` for `'char'`; and `Complex values cannot
  be converted to logicals.`, pinned by `err_if_complex_condition`. A
  complex value a solver's function returns is refused in the solver's
  name, not with cycle 09's `must return a real scalar`, so one message
  covers the start and the value (`err_fzero_complex_value`).
- **A negative-zero imaginary part displays as `+ 0.0000i`**, as the flag
  rule reads it: the conjugate of the real `3` in `[1+2i 3]'`
  (`ctranspose_conjugates`).
- **A zero real part printed through `%.4f` is `0.0000`**, never
  `-0.0000`, in every case that prints one: each is `+0` by the arithmetic
  that makes it.
- **The same input as the refusal.** Scope asks each replacement to test
  the retired case's own input, and item 10 names different ones for three
  of them. Both are kept: `complex_log10_negative`,
  `complex_acos_outside_range` and `complex_power_builtin_fractional` check
  item 10's input and then the refusal's (`log10(-10)`, `acos(-2)`,
  `power(-2, 0.5)`), each self-checking.
- **Retired cases and their replacements**: 01d's `err_complex_sqrt` by
  `complex_sqrt_negative`, `err_complex_log` by `complex_log_negative`,
  `err_complex_log2` by `complex_log2_negative`, `err_complex_log10` by
  `complex_log10_negative`, `err_complex_asin` by `complex_asin_above_one`,
  `err_complex_acos` by `complex_acos_outside_range`,
  `err_complex_power_operator` by `complex_power_operator_fractional`,
  `err_complex_elementwise_power` by `complex_elementwise_power_fractional`
  and `err_complex_power_builtin` by `complex_power_builtin_fractional`; 08's
  `err_eig_complex_eigenvalues` by `eig_complex_eigenvalues_rotation`; 09's
  `err_roots_complex` by `roots_complex_sorted`.

### Deviations accepted

- Every builtin not on `TAKES_COMPLEX` refuses a complex argument where
  MATLAB takes many of them (`max`, `sort`, `reshape`, `num2str`, `inv`,
  `det`, and more); a later cycle can move a builtin onto the list by
  teaching it `im`.
- `if`, `while` and the logical operators refuse a complex value.
- Indexing, concatenation and deletion drop an all-zero imaginary part:
  each builds a new array, so the check costs no more than the result. An
  indexed assignment keeps complex storage complex, even when every
  imaginary part it leaves is zero, and the next arithmetic result drops
  it. Set at review: the first rule rescanned the whole array after every
  write, which made assignment loops quadratic (160,000 complex appends
  took 62 s against 1.1 s for real ones). MATLAB's rule for these was not
  taken from a MathWorks page.
- `eig` and `roots` refuse complex input.
- The complex display's layout past the recorded forms, and the phase of
  complex eigenvectors, are SplatCrab's own.

**Fixed at review.** Indexed assignment into complex storage copied the
whole imaginary array through `imag_part()` and rescanned it on every
write, so assignment loops were quadratic and cycle 03's in-place,
amortised-growth guarantee did not hold for complex values. The imaginary
parts are now scattered in place beside the real ones, and an assignment
keeps complex storage (the deviation above). `complex_growth_perf_guard`
is cycle 03's `growth_perf_guard` with complex values;
`assign_keeps_complex_storage` pins the rule. The real embedding that
complex `\`, `/` and `inv` solve through is four times the input's
elements; its shape is now judged by `check_shape` before it is
allocated, as every other computed shape is.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/10-complex/`. Expected output is written by hand from MATLAB
semantics wherever feasible; the `.err` substring is always chosen by hand.
Every new error message needs an `err_*` case.

1. `z = 3 + 4i; disp(abs(z)); disp(real(z)); disp(imag(z)); disp(conj(z))` → `     5\n     3\n     4\n   3.0000 - 4.0000i` Cases: abs_real_imag_conj, angle_of_complex.
2. `z = sqrt(-4)` → `z =\n\n   0.0000 + 2.0000i\n` Cases: display_complex_scalar.
3. `r = roots([1 0 1]); fprintf('%.4f %.4f\n', imag(r))` → `1.0000 -1.0000` Cases: roots_complex_pair_in_order.
4. `e = eig([0 -1; 1 0]); disp(isreal(e)); fprintf('%.4f\n', abs(e))` → `   0\n1.0000\n1.0000` (`isreal` returns a logical, four wide since cycle 02) Cases: eig_complex_eigenvalues_rotation.
5. `fprintf('%.4f ', abs(fft([1 0 0 0]))); fprintf('\n'); y = fft([1 2 3 4]); fprintf('%.4f ', abs(y)); fprintf('\n'); fprintf('%.4f ', real(ifft(fft([1 2 3])))); fprintf('\n')` → `1.0000 1.0000 1.0000 1.0000 \n10.0000 2.8284 2.0000 2.8284 \n1.0000 2.0000 3.0000 `. Rewritten at planning: the round trip is not exact, so `disp` of it is not `1 2 3`, and whether `fft([1 0 0 0])` displays as real depended on the flag rule item 7 now settles Cases: fft_impulse, fft_moduli_length_four, ifft_round_trip_length_three, fft_prime_length_fast.
6. `disp([1+2i 3]')` → `   1.0000 - 2.0000i\n   3.0000 + 0.0000i`; `disp([1+2i 3].')` → `   1.0000 + 2.0000i\n   3.0000 + 0.0000i` Cases: ctranspose_conjugates, transpose_does_not_conjugate, complex_index_and_assign, err_char_target_complex_assign.
7. `disp(class(1i)); disp(isreal(1i * 0)); disp(isreal(complex(1, 0))); disp(isreal((3+4i) + (5-4i)))` → `double\n   1\n   0\n   1`. The semantics are locked in Scope from MathWorks' pages: arithmetic drops an all-zero imaginary part, `complex` keeps one. The spec first read `     0` for `isreal(1i * 0)`, which those pages contradict Cases: class_of_complex_is_double, isreal_arithmetic_drops_zero_imag, isreal_complex_keeps_zero_imag, complex_arithmetic_operators.
8. `disp((1+2i) == (1+2i)); disp((1+2i) == 1); disp((1+5i) < (2+0i)); disp((3+0i) > (2+9i))` → `   1\n   0\n   1\n   1`: `==` tests both parts, `<` and `>` the real part only Cases: eq_compares_both_parts, ordering_compares_real_parts, ne_compares_both_parts, le_ge_compare_real_parts.
9. `fprintf('%d\n', 3+4i); fprintf('%.2f\n', 2.5-1i)` → `3\n2.50`: numeric conversions print only the real part Cases: printf_prints_real_part, sprintf_prints_real_part.
10. The refusals replaced, one case each, all self-checking or `%.4f`: `disp(sqrt(-4) == 2i)` → `   1`; `fprintf('%.4f %.4f\n', real(log(-1)), imag(log(-1)))` → `0.0000 3.1416`; `disp(abs(2^log2(-8) + 8) < 1e-12)` and `disp(abs(10^log10(-8) + 8) < 1e-12)` → `   1` each; `disp(abs(sin(asin(2)) - 2) < 1e-12)` and `disp(abs(cos(acos(2)) - 2) < 1e-12)` → `   1` each; `z = (-8)^(1/3); fprintf('%.4f %.4f\n', real(z), imag(z))` → `1.0000 1.7321`, the same through `.^` and `power`; `r = sort(imag(roots([1 0 1]))); fprintf('%.4f %.4f\n', r)` → `-1.0000 1.0000` Cases: complex_sqrt_negative, complex_log_negative, complex_log2_negative, complex_log10_negative, complex_asin_above_one, complex_acos_outside_range, complex_power_operator_fractional, complex_elementwise_power_fractional, complex_power_builtin_fractional, roots_complex_sorted.
11. `i = 5; disp(i); clear i; disp(imag(i))` → `     5\n     1`: a variable shadows the imaginary unit, and clearing it brings the unit back Cases: imaginary_unit_shadowed_by_variable, imaginary_literal_forms.
12. `sort([1+2i 3])` → a clean error, exit 1, with SplatCrab's own text pinned in its `err_*` case: a builtin that does not take complex input refuses one rather than drop the imaginary part Cases: err_sort_complex_input, err_max_complex_input, err_min_complex_input, err_floor_complex_input, err_mod_complex_input, err_fzero_complex_start, err_fzero_complex_value, err_if_complex_condition, err_colon_complex_operand, complex_kernels_take_complex_input, sin_cos_imaginary_axis.
13. `c = {1+2i}; s.z = 3i; disp(imag(c{1}) + imag(s.z))` → `     5`; and under `--protocol`, after `z = 1+2i;`, `workspace` lists `z` with class `double` and size `[1,1]` Cases: cell_and_struct_hold_complex, workspace_lists_complex_as_double.

## Status

Done (2026-09-29)
