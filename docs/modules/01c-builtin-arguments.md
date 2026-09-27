# 01c — Builtin arguments

## Goal

Give the builtins the argument forms MATLAB code actually uses. Cycle 01 put
an arity check on every builtin, which turned a family of silently ignored
arguments (`sort(v, 'descend')`, `find(x, k)`, `norm(v, p)`, ...) into honest
"Too many input arguments." errors. This cycle implements those arguments, the
size-vector idiom `zeros(size(A))`, and the small argument defects the Phase 0
QA pass found in the same builtins.

```matlab
A = ones(2, 3);
Z = zeros(size(A));           % 2x3, not "must be a scalar"
v = sort([3 1 2], 'descend'); % 3 2 1
k = find([0 1 1 1], 2);       % 2 3
n = norm([3 -4], 1);          % 7
D = diag([1 2], 1);           % 3x3 with the vector above the diagonal
s = num2str(pi, 8);           % '3.1415927'
r = round(pi, 2);             % 3.14
t = sum(A, 'all');            % 6
```

The first bug-fix cycle of three (01c, 01d, 01e) that run between 01b and 02.
Every row it fixes is in the "Known bugs" table of `docs/ARCHITECTURE.md`.

## Scope

Each bullet fixes one Known bugs row. "QA Dn" names the Phase 0 QA defect.

1. **Constants.** `true` and `false` take sizes exactly as `NaN` and `Inf`
   do: `true(n)`, `true(r, c)`, `true(sz)`. They return 0/1 doubles until cycle
   02 gives them the logical class. `eps(x)` is the spacing at `abs(x)`,
   element-wise over an array; `eps('double')` is `eps`. `pi` keeps its
   single syntax: `pi(2)` stays "Too many input arguments." (the MATLAB `pi`
   page lists only `p = pi`). This bullet replaces the old row, which misstated
   MATLAB (see Design notes).
2. **`e` is removed** (QA D34). MATLAB has no `e` constant; `exp(1)` is the
   MATLAB spelling, and `e` is an Octave extension. `e` becomes an ordinary
   undefined name, still free to be a variable. The registry drops to 80.
3. **`sort` direction.** `sort(v, direction)`, `sort(v, dim)` and
   `sort(v, dim, direction)` for vectors, with `direction` `'ascend'` or
   `'descend'`. Descending is stable and puts `NaN` first.
4. **`find` count.** `find(X, n)` returns the first `n` nonzero indices, and
   `find(X, n, 'first')` and `find(X, n, 'last')` choose the end. The result
   keeps `find`'s orientation, and the last `n` stay in ascending order.
5. **`norm` order.** `norm(v, p)` for vectors: `1`, `2`, any positive real `p`,
   `Inf`, `-Inf`, `'fro'` (the 2-norm) and `'inf'`. A matrix stays "supports
   vectors only" until cycle 08.
6. **`norm` without overflow, and no negative zero** (QA D12). The 2-norm and
   the p-norms scale by the largest magnitude, so `norm([1e200 1e200])` is
   `1.4142e+200` and `norm([1e-200 1e-200])` is `1.4142e-200`, not `Inf` and
   `0`. The sum of no elements is `+0`: `norm([])`, `sum([])` and
   `dot([], [])` currently print `-0.0000`, because Rust's `f64` `Sum` starts
   from `-0.0`.
7. **`diag` offset.** `diag(v, k)` places `v` on the k-th diagonal of a square
   matrix of order `numel(v) + abs(k)`, and `diag(A, k)` returns the k-th
   diagonal of `A` as a column. A `k` past the matrix gives a 0x1.
8. **`num2str` precision.** For a scalar `x`, `num2str(x, n)` formats with
   `%.{n}g`, where `n` is a positive integer, and `num2str(x, formatSpec)` is
   `sprintf(formatSpec, x)` with leading whitespace trimmed.
9. **`round` digit count.** `round(x, n)` rounds to the nearest multiple of
   `10^-n` for any integer `n`, ties away from zero. `round(x, n, 'decimals')`
   is the same, and `round(x, n, 'significant')` rounds to `n` significant
   digits, where `n` must be positive.
10. **`max` and `min` on an empty.** They follow MATLAB's documented rule. The
    reduced dimension becomes 1 unless its size is 0, and then the result has
    the input's size. So `max(zeros(3, 0))` is 1x0, `max(zeros(0, 3))` is 0x3
    and `max([], [], 1)` is 0x0. This corrects the old row, which claimed the
    last was 1x0.
11. **A size past `usize` is named as asked.** The size-overflow message names
    the size the user requested, not the `usize::MAX` clamp:
    `zeros(1e300)` reports `Requested 1e+300x1e+300 array exceeds the maximum
    array size.`, and `0:1e-300:1e300`, whose count overflows `f64`, reports
    `1xInf`. Indexed growth (`x(1e300) = 1`) is out of scope (cycle 03).
12. **Trailing singleton sizes.** `zeros`, `ones`, `rand`, `NaN`, `Inf`,
    `true`, `false`, `reshape` and `repmat` accept any number of trailing size
    arguments (or size-vector elements) equal to `1`. Any other third or later
    size, including `0`, is the clean error `N-D arrays are not supported.`
    This does not build N-D arrays, so that Known bugs row stays open. `eye`
    still takes at most two sizes.
13. **`dot` of matrices** (QA D10). `dot(A, B)` of two same-size matrices
    returns the dot products of corresponding columns, along the first
    non-singleton dimension, and `dot(A, B, dim)` chooses the dimension. Two
    vectors of equal length may differ in orientation. Anything else of
    unequal size is an error, where today `dot(ones(2, 3), ones(3, 2))` is `6`.
14. **`any` ignores `NaN`** (QA D11). `any(NaN)` is `0`, because the MATLAB
    `any` page says "any ignores elements of A that are NaN". Octave says `1`,
    and MATLAB wins.
15. **`isvector` of an empty vector** (QA D18). A 1x0 or 0x1 array is a vector
    ("1-by-N or N-by-1, where N is a nonnegative integer"). A 0x0 is not.
16. **`toc` before `tic`** (QA D19). A bare `toc`, or `x = toc`, with no
    earlier bare `tic` is an error. `t = tic` does not count as a bare `tic`.
    `toc(t)` is unaffected.
17. **Size vectors** (QA D21). `zeros(sz)`, `ones(sz)`, `eye(sz)`, `rand(sz)`,
    `NaN(sz)`, `Inf(sz)`, `true(sz)` and `false(sz)` accept a row vector of
    sizes, so `zeros(size(A))` works. `reshape(A, sz)` does too, and so do
    `reshape(A, r, [])` and `reshape(A, [], c)`, with one `[]` placeholder for
    the size that makes the count come out. `repmat(A, sz)` accepts one as
    well.
18. **`linspace` floors its count** (QA D23). A non-integer `n` gives
    `floor(n)` points, as the MATLAB page says.
19. **Option strings are not dimensions** (QA D24). A char argument where a
    dimension or a size is expected is never read as its character codes:
    `sum(A, 'x')` is an error, where today it is the input unchanged
    (dimension 120). `'all'` reduces over every element in `sum`, `prod`,
    `mean`, `any` and `all`, and in `max(A, [], 'all')` and
    `min(A, [], 'all')`.

## Out of scope

- N-D arrays. Bullet 12 turns the silent loss of a third size into an honest
  error. It does not build `2x3x4` arrays, and no roadmap module does yet.
- Class-name arguments: `zeros(2, 'int8')`, `eps('single')`,
  `true(2, 'like', p)`. They need the classes of cycle 02 and beyond.
  `eps('double')` is the one exception, because it names the only class that
  exists.
- `num2str` of a non-scalar, with or without a precision or a format (QA
  D13). MATLAB returns one char row per matrix row, with precision-dependent
  column widths, and a multi-row char needs the char matrices of cycle 02. It
  is cycle 11's, next to the rest of `num2str`. Until then a non-scalar keeps
  today's one-row output, and no golden case asserts its spacing.
- Matrix `sort` and `sort(A, dim)` for a matrix (Known deviations, cycle 09),
  and the second output `[s, i] = sort(...)` (cycle 03).
- Matrix `norm` (cycle 08).
- `fprintf(fid, ...)` and `nbytes = fprintf(...)` (QA D25). Cycle 11 owns
  `fprintf(fid, ...)`, and a file id of 2 needs a stderr sink on `Interp` that
  cycle 04's `warning` introduces first. Moved there at this cycle's planning, for
  size.
- Trailing singleton subscripts `A(2, 1, 1)` (QA D22). This is indexing, not a
  builtin argument, and cycle 03 rewrites index resolution. Moved there.
- The size message for indexed growth, `x = []; x(1e300) = 1`, which reaches
  `check_size` through `eval_index_args` and still names `usize::MAX`. Cycle 03
  rewrites `assign_index`; moved there.
- `find([])` and `diag([])` giving 0x1 where MATLAB gives 0x0. These are the
  empty-result-shapes row, which cycle 01e owns.
- `'omitnan'`, `'includenan'`, `'native'` and `'double'` options on the
  reductions, and vector dimensions (`vecdim`). They are clean errors after
  bullet 19, as every unrecognised char option is.

## Design notes

### Sources for the behaviour

Values and shapes were checked against GNU Octave 8.4. They were checked
against the MathWorks reference pages where the two might differ, and MATLAB
wins where they do.

- **Constants.** The `pi` page lists one syntax, `p = pi`. The `true` page:
  `true(n)`, `true(sz)` and `true(sz1,...,szN)`; "If n is negative, then it is
  treated as 0"; "If any trailing dimensions greater than 2 have a size of 1,
  then the output ... does not include those dimensions". The `eps` page:
  "the positive distance from abs(x) to the next larger floating-point
  number"; "If x is Inf or NaN, then eps(x) returns NaN". Octave gives
  `eps(0) = 4.94066e-324` (the smallest subnormal) and
  `eps(1e308) = 1.99584e+292`.
- **The old constants row was wrong three ways.** It said `pi(2)`, `e(2)` and
  `eps(2)` fill a 2x2. `pi(2)` is an error in MATLAB, `e` does not exist
  there, and `eps(2)` is `4.4409e-16`. Only `true` and `false` fill.
- **`max` and `min` on an empty.** The `max` page: "If A is a 0-by-0 empty
  array, then max(A) is as well", and "If size(A,dim) is 0, then max(A,dim)
  returns an empty array with the same size as A". Octave agrees on every
  case in acceptance test 10. The old row's claim that `max([], [], 1)` is 1x0
  was an analogy with `sum`, whose empty reduces to the identity element, `0`.
  `max` has no identity element, so the analogy fails.
- **`sort`.** The `sort` page: `'MissingPlacement'` defaults to `'auto'`,
  "placed last for ascending order and first for descending order", and "The
  sort function uses a stable sorting algorithm ... regardless of sorting
  direction".
- **`find`.** The `find` page: `n` is "a positive integer scalar"; `'first'`
  or `'last'`; "If X is a row vector, then k is also a row vector", otherwise
  a column. Octave accepts `n = 0`; MATLAB's page does not, and MATLAB wins.
- **`norm`.** The `norm` page: "p ... 2 (default), a positive real scalar, Inf,
  or -Inf"; "The norm of an empty matrix is zero". It does not say what `p = 0`
  or a negative finite `p` does. This cycle rejects both, and no golden case
  asserts either, because MATLAB's runtime behaviour for them is unverified.
- **`diag`.** The `diag` page: "a square matrix of order N+abs(k)", and "k>0 is
  above the main diagonal". It is silent on a `k` past the matrix. Octave gives
  0x1, which is also MATLAB's shape for an empty diagonal, and that is what
  this cycle does.
- **`num2str`.** The `num2str` page: "precision — Maximum number of significant
  digits ..., specified as a positive integer", and "num2str trims any leading
  spaces from a character array, even when formatSpec includes a space
  character flag", with the example `num2str(42.67,'% 10.2f')` giving `'42.67'`.
  Octave gives `num2str(pi, 8) = 3.1415927`,
  `num2str(123456, 3) = 1.23e+05` and
  `num2str(pi, 100) = 3.141592653589793115997963468544185161590576171875`,
  the exact binary expansion.
- **`round`.** The `round` page: "N — Number of digits, specified as a scalar
  integer ... rounds X to the nearest multiple of 10^-N"; the type is
  `"decimals"` (default) or `"significant"`, where N must be positive;
  ties go "away from zero". It gives the examples `round(pi,3)` = `3.1420`,
  `round(863178137,-2)` = `863178100` and
  `round([1253 1.345 120.44],2,"significant")` = `[1300 1.3 120]`.
  Octave 8.4 has no `round(x, n)`, so the MATLAB page is the only source.
- **`dot`.** The `dot` page: for matrices, "the dot product of corresponding
  vectors along the first array dimension whose size does not equal 1";
  "A and B must have the same size" (vectors need only the same length).
  Octave: `dot([1 2; 3 4], [1 2; 3 4])` is `10 20`, with `dim = 2` it is
  `5; 25`, and `dot([1 2 3], [4; 5; 6])` is `32`.
- **`any`.** The `any` page: "any ignores elements of A that are NaN". Octave
  disagrees, and MATLAB wins.
- **`linspace`.** The `linspace` page: "If n is not an integer, linspace
  rounds down and returns floor(n) points", and "If n is zero or negative,
  linspace returns an empty 1-by-0 matrix".
- **`'all'`.** The `sum`, `any` and `max` pages list `sum(A,"all")`,
  `any(A,'all')` and `max(A,[],"all")` (R2018b and later). Octave 8.4 has none
  of them.
- **`toc`.** The `toc` page is silent about a missing `tic`. The message below
  is MATLAB's, from knowledge rather than from a page; Octave also errors.
- **`reshape`.** Octave: `reshape(1:6, [], 2)` is 3x2. `reshape(1:5, [], 2)`
  and a second `[]` are errors. The two message texts below are MATLAB's,
  from knowledge.

### New error messages

All live in `src/error.rs`, like every other message. The texts marked
"MATLAB" are MATLAB's own wording, from knowledge; the rest are SplatCrab's.

| Where | Text |
|---|---|
| a third or later size other than 1 | `N-D arrays are not supported.` |
| a size vector that is not a row | `Size vector for '<name>' must be a row vector.` |
| `reshape` placeholder that does not divide | `Product of known dimensions, <p>, not divisible into total number of elements, <n>.` (MATLAB) |
| `reshape` with two placeholders | `Size can only have one unknown dimension.` (MATLAB) |
| `sort` direction | `Sort direction for 'sort' must be 'ascend' or 'descend'.` |
| `find` count | `Number of elements for 'find' must be a positive integer scalar.` |
| `find` direction | `Search direction for 'find' must be 'first' or 'last'.` |
| `norm` type | `Norm type for 'norm' must be a positive real scalar, Inf, -Inf or 'fro'.` |
| `diag` offset | `K-th diagonal input must be an integer scalar.` (MATLAB) |
| `num2str` precision | `Precision for 'num2str' must be a positive integer.` |
| `round` digits | `Number of digits for 'round' must be an integer scalar.` |
| `round` significant digits | `Number of significant digits for 'round' must be a positive integer scalar.` |
| `round` type | `Rounding type for 'round' must be 'decimals' or 'significant'.` |
| `toc` before `tic` | `You must call TIC without an output argument before calling TOC without an input argument.` (MATLAB) |
| `dot` sizes | `A and B must be the same size for 'dot'.` It replaces `Vectors must be the same length for 'dot'.`, which no golden case asserts |
| `eps` with a class name other than `'double'` | `Only 'double' is supported as a class name for 'eps'.` |

A char where a dimension is expected reuses `Dimension argument to '<name>'
must be a positive integer scalar.`, and a char where a size is expected
reuses `Size arguments to '<name>' must be non-negative integers.`. Neither is
new.

### How to build it

These are guidance for the implementation. Record the decisions actually taken
under "Decisions taken while building it" below.

- **One size parser.** Every constructor, the size forms of `NaN`, `Inf`,
  `true` and `false`, and `reshape` and `repmat` read their sizes through one
  helper in `args.rs`. It accepts no size (1x1), a scalar `n` (n x n), a row
  vector, or two or more scalars, and applies the trailing-ones rule of bullet
  12 in one place. `eye` asks it for at most two.
- **Keep a requested size as `f64` until `check_size` judges it,** so that the
  message can name it (bullet 11). The rule for printing one dimension: an
  integer below 2^53 prints in full, so every existing message is unchanged,
  including `10000000000x10000000000` and `1x1000000000000000`. Anything else
  prints in `%g` form (`1e+300`), and an infinite count prints `Inf`. The `:`
  operator goes through the same path.
- **`eps(x)` from the exponent field,** not from next-float subtraction. The
  successor of the largest double is `Inf`, and `eps(1e308)` must be
  `2^971`, not `Inf`. Zero and subnormals give `2^-1074`.
- **`round(x, n)`** is `round(x * 10^n) / 10^n` for `n > 0` and
  `round(x / 10^-n) * 10^-n` for `n < 0`, with two guards. If `10^|n|` or the
  scaled value is not finite, or the scaled value is at least 2^52 (already an
  integer at that scale), the answer is `x` itself for `n > 0`. For `n < 0` it
  is `0`, which is what `round(5, -400)` must give, not `NaN`. `'significant'`
  converts to decimals: `n - floor(log10(abs(x))) - 1`. `0`, `Inf` and `NaN`
  pass through. Unit tests with tolerances cover all of this in `math.rs`.
- **Scaled norms.** Take `s = max(abs(v))`. For a finite, nonzero `s` the norm
  is `s * (sum((abs(v) / s).^p))^(1/p)`. Otherwise `s` is the answer: `0` for
  an all-zero or empty vector, `Inf` if any element is infinite, `NaN` if any
  is `NaN`. `p = 1` and `p = ±Inf` need no scaling. Sum with `fold(0.0, ..)`,
  not `.sum()`, wherever an empty sum can print. That covers `reduce`'s `sum`
  and `mean`, `norm` and `dot`.
- **`sort` descending** is `sort_by(|a, b| sort_cmp(b, a))`. It stays stable
  and puts `NaN` first, which is the documented placement.
- **`find(X, n, 'last')`** takes the last `n` indices and keeps them in
  ascending order, as MATLAB returns them.
- **`dot`.** Two vectors of equal `numel` in any orientation give a scalar.
  Otherwise the sizes must be equal, and the answer is `sum(A .* B)` along the
  default or given dimension, which reuses `math::reduce`. With `dim`, even two
  vectors must be the same size.
- **`toc`** needs to know whether a bare `tic` has run. `tic_mark` becomes an
  `Option`.
- **`isvector`.** Either change `Matrix::is_vector` or add a predicate for the
  builtin alone. If `Matrix::is_vector` changes, check its other callers
  (`diag`, `sort`, `norm`, `dot`): `diag(zeros(1, 0))` would then be 0x0,
  which Octave agrees with.
- **`'all'`** is the reduction over `A(:)`. Recognise it only where bullet 19
  lists it. Anywhere else a char in a dimension position is the existing
  dimension error.
- **`num2str(x, n)`** needs no new bound, because `%g` output cannot exceed
  about 770 digits. Clamp `n` internally to a value no double can tell apart,
  such as 800, before calling `fmt_g`, so `num2str(pi, 1e9)` neither panics nor
  allocates. Do not change `fmt_g` or the `printf` path: their precision
  bounds are cycle 01d's.
- **Registry.** `EXPECTED` becomes 80, and `e` leaves the spot-check list. Help
  strings change for `true`, `false`, `eps`, `sort`, `find`, `norm`, `diag`,
  `num2str`, `round`, `dot` and `isvector`.

### Notes for the tests

- `true(n)` returns doubles until cycle 02, and `disp` of a double 0/1 is six
  characters wide where MATLAB's logical display is four. Assert `true` and
  `false` through `size` and `sum`, never through `disp` of the values.
- A `NaN` or `Inf` element still forces a row to four decimals (cycle 01e).
  Assert any result containing one through `fprintf('%g ')`.
- `e` is removed, but cycle 01e may reword "Undefined function or variable".
  Choose the `.err` substring `function or variable 'e'`, which survives
  either wording.
- Avoid rounding ties in the `round` cases, and use `fprintf('%.4f')` for any
  non-integer result.

### Decisions taken while building it

**The size parser** (`src/builtins/args.rs`). It is three layers, so that
`reshape` can use the lower two without the constructors' `n`-by-`n` rule:

- `size_list(args, from, name, auto)` reads the raw list. One scalar `n` is
  `[n, n]`. One row vector gives its elements. One empty argument, of any
  shape, is `[0, 0]`: `zeros([])` and `zeros(zeros(1, 0))` are 0x0, as in
  Octave. One column or matrix is `Size vector for '<name>' must be a row
  vector.`, where Octave accepts `zeros([2; 3])` and MATLAB does not. Two or
  more arguments must each be a scalar. The existing `Argument 2 to 'zeros'
  must be a scalar.` stays for `zeros(2, [3 4])` and `zeros(2, [])`. With
  `auto`, an empty argument among several is the `[]` placeholder, returned as
  `None`. Every element goes through `size_value`: `NaN`, a fraction or an
  infinity is the size error, and a negative size is `0`. A char, in any
  position, is the size error.
- `trailing_ones(dims)` drops the third and later sizes when every one is
  `1`, and otherwise returns `N-D arrays are not supported.`.
- `shape(args, from, name, max_dims)` is what the constructors, `NaN`, `Inf`,
  `true`, `false` and `repmat` call. It returns 1x1 for no sizes. `eye`
  passes `max_dims = 2`, as well as keeping `at_most(args, 2)`. So
  `eye(2, 3, 1)` is still `Too many input arguments.`, and `eye([2 3 1])` is
  the N-D error even though its third size is 1. The `eye` page allows a size
  vector of "no more than two integer values".

**Requested sizes stay `f64`.** `size_arg` and `size_value` now return `f64`.
The new `check_shape(rows: f64, cols: f64) -> R<(usize, usize)>` is the only
place a requested shape becomes lengths. It is used by the constructors,
`linspace`, `diag`, `reshape`, `repmat` and the `:` operator. `fmt_dim`
renders one dimension for the message: an integer below 2^53 in full, and
anything else through `fmt_g(v, 6)`, which gives `1e+300`, `9.0072e+15` and
`Inf`. `error::size_overflow` now takes the two dimensions already rendered,
as `&str`. The old `check_size(usize, usize)` stays for indexed growth and
renders its `usize` values in full, so `x(1e300) = 1` still names
`18446744073709551615` until cycle 03. A dimension that does not fit in a
`usize` is refused even when the other is `0`: `zeros(0, 1e300)` now reports
`Requested 0x1e+300 array ...`. It used to build a 0x18446744073709551615
matrix from the saturated value. Octave refuses it too.

**`linspace`** floors `n` and clamps it at `0`. A `NaN` count gives 1x0, as in
Octave. An infinite count reaches `check_shape` and reports `1xInf`. A char
count is the size error.

**`eps`** reads the exponent field (`eps_at`, `pow2` in `core.rs`), so every
spacing is exact. The unit test checks `x + eps(x)` against the next double
up. `eps('double')` matches the class name case-insensitively, as Octave
does. Any other char is `Only 'double' is supported as a class name for
'eps'.`.

**`e`** is gone from the registry. `EXPECTED` is 80, and the registry test now
also asserts that `e` is absent.

**Option strings are matched case-insensitively.** This covers `'ascend'`,
`'descend'`, `'first'`, `'last'`, `'fro'`, `'inf'`, `'decimals'`,
`'significant'`, `'all'` and `'double'`. MATLAB matches these options without
regard to case. That is from knowledge, not from a page. Octave's `sort` and
`find` are case-sensitive, and its `norm` and `eps` are not. No golden case
asserts a case variant.

**`sort`.** With two arguments, a char is the direction and anything else is a
dimension. With three, the second is a dimension and the third must be a
direction; a numeric third argument is the direction error. The options are
parsed before the vectors-only check, so a bad option is reported first.
Along a dimension the vector does not extend in (`sort([3 1 2], 1)`, or any
`dim >= 3`), nothing moves. Descending is `sort_by(|a, b| sort_cmp(b, a))`, as
planned.

**`find`.** `n` must be a finite positive integer, so `find(x, Inf)` is
refused. The page says "positive integer scalar"; Octave accepts `Inf`. A `n`
past the number of nonzeros returns all of them. With three arguments, a
numeric third is the direction error.

**`norm`.** `p` is parsed before the vectors-only check. A `NaN` element gives
`NaN` for every `p`, including `-Inf`, as Octave does for `norm([Inf NaN])`
and `norm([NaN 1], -Inf)`. The `-Inf` norm of an empty is `0`, as in Octave.
`p = 2` scales and squares as `(x/s)*(x/s)`, then takes `sqrt`, rather than
calling `powf`, so the common case loses no accuracy. `p = 1` and `p = Inf`
need no scaling. `vector_norm` is the unit-tested kernel.

**Empty sums.** `math::sum0` folds from `+0`, and `sum`, `mean` and `dot`
use it. `norm` folds from `0.0` directly. A non-empty sum is unchanged, except
that `sum(-0)` is now `+0`. Octave gives `1/sum(-0) = Inf` too.

**`isvector`.** `Matrix::is_vector` changed, rather than a builtin-only
predicate being added, to `rows == 1 || cols == 1`. The other callers are
affected as follows.

- `diag`: `diag(zeros(1, 0))` is now 0x0 and `diag(zeros(1, 0), 1)` is a 1x1
  zero. Octave agrees with both.
- `norm`, `sort`: no change, since both already let an empty through.
- `dot`: two empty vectors of equal length give `0`.
- `index_read` in `interp.rs`: a 0x1 indexed by a 1x0 now keeps the source's
  orientation and gives 0x1, where it gave 1x0. Octave gives 0x1. This is an
  indexing change, and cycle 03's territory, reached only through an empty
  index into an empty vector. No golden case asserts either shape.

**`diag`.** `k` must be a finite integer scalar. A char, a non-scalar, a
fraction, `NaN` and `Inf` are all `K-th diagonal input must be an integer
scalar.`. In the matrix branch, the diagonal's start stays `f64` until it is
known to lie inside the matrix, so `diag(A, 1e300)` is a clean 0x1. In the
vector branch the order `numel(v) + abs(k)` goes through `check_shape`, so
`diag([1 2], 1e300)` reports `1e+300x1e+300`.

**`num2str`.** The precision is clamped at 800 before `fmt_g`, and neither
`fmt_g` nor the `printf` path changed. A char first argument comes back
unchanged whatever the second argument is, as MATLAB returns a char input. A
non-scalar with a precision formats each element with `%.{n}g` and joins them
with two spaces. With a format, it is `sprintf(formatSpec, x)` over every
element in column-major order. Both extend today's one-row output, and no
golden case asserts either. The trim is `trim_start`, all leading whitespace.

**`round`.** The type is parsed first, then `n`. With `'significant'`, any bad
`n`, a fraction as well as `0` or a negative, gives the significant-digits
message. `10^k` is computed by parsing `"1e{k}"`, with `k` clamped to ±400.
The parser is correctly rounded, and it gives `Inf` or `0` past the range,
where `powi` accumulates error. The guards are the planned ones. For
`n < 0` there is one addition: an `x / 10^-n` of at least 2^52 is already a
multiple of the step, so `x` itself is returned. That makes
`round(1e300, -2)` `1e300`, not the `0` a literal reading of the guard would
give. `round(-5, -400)` is `+0`.

**`max` and `min`.** The default dimension is the first non-singleton:
`rows == 1 ? 2 : 1`, which is dimension 1 for a 0x0. `extremum_along` returns
the input unchanged when the reduced dimension has size 0, and otherwise
calls `reduce`. A `dim >= 3` has size 1, so an empty keeps its shape there,
as Octave's `max(zeros(0, 3), [], 3)` does. `max([], [], 'all')` is the 0x0
`[]`: `max` has no identity element, and the MATLAB page is silent. No
golden case asserts it.

**`dot`.** The vector shortcut applies only without `dim`. `dot(A, B, dim)`
with `dim >= 3` is `A .* B`, as in Octave, because `reduce` returns its input
for such a dimension.

**`'all'`** is `args::dim_or_all`, called only by `sum`, `prod`, `mean`,
`any`, `all`, `max` and `min`. `args::dim` now rejects every char, so
`cumsum(A, 'all')`, `size(A, 'x')`, `sort(v, 'x', 'ascend')` and
`dot(a, b, 'x')` are all the dimension error.

**`toc`.** `Interp.tic_mark` is an `Option<f64>`, set only by a bare `tic`.

**`reshape`.** A single size argument that is a scalar or empty keeps
`Not enough input arguments for 'reshape'.`. MATLAB's text, "Size vector must
have at least two elements.", is not in the message table, so it is not
added. Any empty numeric argument is a placeholder, which is what Octave
accepts. When the known sizes multiply to `0`, an empty input resolves the
placeholder to `0`, as Octave's `reshape(zeros(0, 3), [], 0)` does, and a
non-empty input is the not-divisible error. A negative size is still `0`, so
it ends in the element-count error. Octave rejects it outright; that is
unchanged behaviour, not a decision of this cycle. The count check keeps the
existing `To reshape the number of elements must not change` message.

**`repmat`.** The requested shape, `size(A) .* [r c]`, is computed in `f64`
and judged by `check_shape`. So `repmat([1 2], 1e10, 1e10)` still names
`10000000000x20000000000`, and `repmat(zeros(1, 0), 1e300)` is now an error,
as in Octave, where it used to succeed through the saturated size.

**`any` and Octave: a correction.** Scope bullet 14 ("Octave says `1`") and
the `any` source note above ("Octave disagrees") are wrong about Octave. GNU
Octave 8.4 gives `any(NaN) = 0` and `any([NaN NaN]) = 0`, and agrees with
every value in acceptance test 14, `all(NaN) = 1` included. Octave and the
MATLAB page agree, so there was no conflict for MATLAB to win. The behaviour
and the expected output (`0`) are unchanged. The tests found this
first and recorded it in a `% NOTE` in `any_ignores_nan.m`, and
`docs/FEATURES.md` says the same. Scope stays as it was written before any
code; this note is the correction.

**Found and left alone.** `trace([])` still sums with Rust's `Sum` and prints
`-0.0000` under `%.4f`. It is the same defect as bullet 6, but `trace` is not
among the builtins that bullet names.

## Acceptance tests

Each becomes at least one golden case in `tests/cases/01c-builtin-arguments/`,
plus one `err_*` case for every message in the table above. Expected output is
written by hand from the sources in Design notes.

1. Constants:
   `disp(size(true(2))); disp(size(false(2, 3))); disp(size(true([1 4]))); disp(size(false(-1))); disp(sum(sum(true(3))))`
   → `     2     2` / `     2     3` / `     1     4` / `     0     0` / `     9`.
   `fprintf('%g %g %g %g\n', eps(1), eps(2), eps(-2), eps(1e10)); fprintf('%g %g %g %g\n', eps(0), eps(1e308), eps(Inf), eps(NaN)); disp(eps([1 2; 4 8]) / eps); disp(eps('double') == eps)`
   → `2.22045e-16 4.44089e-16 4.44089e-16 1.90735e-06` / `4.94066e-324 1.99584e+292 NaN NaN` / `     1     2` / `     4     8` / `     1`.
   `pi(2)` → err `Too many input arguments.`; `eps('single')` → err
   `Only 'double' is supported as a class name for 'eps'.`
2. `disp(e)` → err containing `function or variable 'e'`, and `e = 5; disp(e)`
   still prints `     5`.
3. `disp(sort([3 1 2], 'descend')); fprintf('%g ', sort([3 NaN 1 2], 'descend')); fprintf('\n'); disp(sort([3 1 2], 'ascend')); disp(sort([3; 1; 2], 'descend')'); disp(sort([3 1 2], 2, 'descend')); disp(sort([3 1 2], 1))`
   → `     3     2     1` / `NaN 3 2 1 ` / `     1     2     3` / `     3     2     1` / `     3     2     1` / `     3     1     2`.
   `sort([1 2], 'up')` → err `'ascend' or 'descend'`.
4. `disp(find([0 1 1 1 0 1], 2)); disp(find([0 1 1 1 0 1], 2, 'last')); disp(find([0 1 1 1 0 1], 2, 'first')); disp(find([0; 1; 1], 5)'); disp(size(find([0; 1; 1], 1)))`
   → `     2     3` / `     4     6` / `     2     3` / `     2     3` / `     1     1`.
   `find([1 1], 0)` → err `positive integer scalar`; `find([1 1], 1, 'middle')`
   → err `'first' or 'last'`.
5. `fprintf('%.4f %.4f %.4f %.4f %.4f %.4f %.4f\n', norm([3 -4], 1), norm([3 -4], 2), norm([3 -4], Inf), norm([3 -4], -Inf), norm([3 -4], 3), norm([3 -4], 'fro'), norm([3 -4], 'inf'))`
   → `7.0000 5.0000 4.0000 3.0000 4.4979 5.0000 4.0000`.
   `norm([1 2], 'abc')` → err `Norm type for 'norm'`.
6. `fprintf('%g %g %g\n', norm([1e200 1e200]), norm([1e-200 1e-200]), norm([1e200 1e200], 3)); fprintf('%.4f %.4f %.4f\n', norm([]), sum([]), dot([], [])); fprintf('%g\n', 1 / sum([]))`
   → `1.41421e+200 1.41421e-200 1.25992e+200` / `0.0000 0.0000 0.0000` / `Inf`.
7. `disp(diag([1 2], 1)); disp(diag([1 2], -1))`
   → `     0     1     0` / `     0     0     2` / `     0     0     0` /
   `     0     0     0` / `     1     0     0` / `     0     2     0`.
   `A = [1 2 3; 4 5 6; 7 8 9]; disp(diag(A, 1)'); disp(diag(A, -2)); disp(size(diag(A, 3))); disp(diag([1 2 3; 4 5 6], -1)); disp(diag(5, 1))`
   → `     2     6` / `     7` / `     0     1` / `     4` / `     0     5` / `     0     0`.
   `diag([1 2], 1.5)` → err `K-th diagonal input must be an integer scalar.`
8. `disp(num2str(pi, 8)); disp(num2str(pi, 2)); disp(num2str(123456, 3)); disp(num2str(7, 3)); disp(num2str(-0.5, 3)); disp(num2str(Inf, 3)); disp(num2str(1/3, 20)); disp(num2str(pi, '%10.4f')); disp(num2str(pi, 100))`
   → `3.1415927` / `3.1` / `1.23e+05` / `7` / `-0.5` / `Inf` /
   `0.33333333333333331483` / `3.1416` /
   `3.141592653589793115997963468544185161590576171875`.
   `num2str(pi, 0)` → err `Precision for 'num2str' must be a positive integer.`
9. `fprintf('%.4f %.4f %.4f %.4f\n', round(pi, 2), round(pi, 3), round(-pi, 1), round(pi, 2, 'decimals')); fprintf('%d %d\n', round(863178137, -2), round(1234, -1)); fprintf('%g %g %g\n', round([1253 1.345 120.44], 2, 'significant')); disp(round(pi, 20) == pi); disp(round(1e307, 2) == 1e307); disp(round(5, -400)); disp(round(2.5, 0))`
   → `3.1400 3.1420 -3.1000 3.1400` / `863178100 1230` / `1300 1.3 120` /
   `     1` / `     1` / `     0` / `     3`.
   `round(pi, 1.5)` → err `Number of digits for 'round' must be an integer scalar.`;
   `round(pi, 0, 'significant')` → err `positive integer scalar`;
   `round(pi, 2, 'banker')` → err `'decimals' or 'significant'`.
10. `disp(size(max(zeros(3, 0)))); disp(size(max(zeros(0, 3)))); disp(size(max(zeros(0, 3), [], 2))); disp(size(min(zeros(3, 0), [], 2))); disp(size(max([], [], 1))); disp(size(min(zeros(1, 0))))`
    → `     1     0` / `     0     3` / `     0     1` / `     3     0` /
    `     0     0` / `     1     0`.
11. `zeros(1e300)` → err containing `Requested 1e+300x1e+300 array`;
    `x = 0:1e-300:1e300;` → err containing `Requested 1xInf array`. Both exit
    1, not 101.
12. `disp(size(zeros(2, 3, 1))); disp(size(ones(2, 3, 1, 1))); disp(size(rand([2 3 1]))); disp(size(NaN(1, 2, 1))); disp(size(true(2, 2, 1))); disp(size(reshape(1:6, 3, 2, 1))); disp(size(repmat(1, 2, 3, 1)))`
    → `     2     3` / `     2     3` / `     2     3` / `     1     2` /
    `     2     2` / `     3     2` / `     2     3`.
    `zeros(2, 3, 4)` → err `N-D arrays are not supported.`
13. `disp(dot([1 2; 3 4], [1 2; 3 4])); disp(dot([1 2; 3 4], [1 2; 3 4], 2)'); disp(dot([1 2 3], [4; 5; 6])); disp(dot([1 2], [3 4], 1)); disp(dot([1 2], [3 4], 2))`
    → `    10    20` / `     5    25` / `    32` / `     3     8` / `    11`.
    `dot(ones(2, 3), ones(3, 2))` → err `A and B must be the same size for 'dot'.`
14. `disp(any(NaN)); disp(any([NaN 0])); disp(any([NaN 1])); disp(any([NaN; 0], 1)); disp(any([NaN 0; 0 2], 2)'); disp(all(NaN))`
    → `     0` / `     0` / `     1` / `     0` / `     0     1` / `     1`.
15. `disp([isvector(zeros(1, 0)) isvector(zeros(0, 1)) isvector([]) isvector(5) isvector(zeros(2, 0)) isvector([1 2 3])])`
    → `     1     1     0     1     0     1`.
16. `toc` as the first statement → err `You must call TIC without an output
    argument before calling TOC without an input argument.`; `t = tic; x = toc`
    → the same error; `tic; x = toc; disp(x >= 0); t = tic; disp(toc(t) >= 0)`
    → `     1` / `     1`.
17. `A = ones(2, 3); disp(size(zeros(size(A)))); disp(size(ones([3 1]))); disp(eye([2 3])); disp(size(rand([2 3]))); disp(size(NaN([2 3]))); disp(size(Inf([1 4]))); disp(size(true([2 2]))); disp(size(zeros([4])))`
    → `     2     3` / `     3     1` / `     1     0     0` / `     0     1     0` /
    `     2     3` / `     2     3` / `     1     4` / `     2     2` / `     4     4`.
    `disp(reshape(1:6, [3 2])); disp(size(reshape(1:6, [], 2))); disp(size(reshape(1:6, 2, []))); disp(repmat([1 2], [2 2])); disp(size(repmat(1, [2 3])))`
    → `     1     4` / `     2     5` / `     3     6` / `     3     2` /
    `     2     3` / `     1     2     1     2` / `     1     2     1     2` /
    `     2     3`.
    `reshape(1:5, [], 2)` → err `Product of known dimensions, 2, not divisible
    into total number of elements, 5.`; `reshape(1:6, [], [])` → err `Size can
    only have one unknown dimension.`; `zeros([2; 3])` → err `Size vector for
    'zeros' must be a row vector.`
18. `disp(size(linspace(0, 1, 2.7))); disp(linspace(0, 1, 2.7)); disp(size(linspace(0, 1, 0.5))); disp(linspace(0, 10, 3.9))`
    → `     1     2` / `     0     1` / `     1     0` / `     0     5    10`.
19. `A = [1 2; 3 4]; disp(sum(A, 'all')); disp(prod(A, 'all')); fprintf('%.4f\n', mean(A, 'all')); disp(any([0 0; 0 NaN], 'all')); disp(all(A, 'all')); disp(max(A, [], 'all')); disp(min(A, [], 'all'))`
    → `    10` / `    24` / `2.5000` / `     0` / `     1` / `     4` / `     1`.
    `sum([1 2; 3 4], 'x')` → err `Dimension argument to 'sum' must be a
    positive integer scalar.`
20. Unit tests: `eps` at `0`, a subnormal, `1`, `1e308`, `-2`, `Inf` and `NaN`;
    `round` at both guards, both signs of `n`, and `'significant'`; the scaled
    norm against the naive formula where both are finite; the size parser's
    trailing-ones rule and the requested-size formatting at 2^53 and beyond.

### Case files

56 cases in `tests/cases/01c-builtin-arguments/`: 21 script cases and 35
`err_*` cases, one or more for every message in the table above.

| Test | Cases |
|---|---|
| 1 | `constants_true_false_sizes`, `eps_spacing`, `err_pi_takes_no_size`, `err_eps_class_name` |
| 2 | `e_is_an_ordinary_name`, `err_e_undefined`, `err_e_undefined_after_clear` |
| 3 | `sort_direction`, `sort_descend_stable`, `err_sort_direction` |
| 4 | `find_count`, `err_find_count_zero`, `err_find_count_fraction`, `err_find_direction` |
| 5 | `norm_order`, `err_norm_type` |
| 6 | `norm_scaled_and_empty_sum` |
| 7 | `diag_offset`, `err_diag_offset` |
| 8 | `num2str_precision`, `err_num2str_precision` |
| 9 | `round_digits`, `err_round_digits`, `err_round_significant_digits`, `err_round_type` |
| 10 | `max_min_empty_shape` |
| 11 | `err_size_overflow_named`, `err_size_overflow_range_inf`, `err_size_overflow_g_form` |
| 12 | `trailing_singleton_sizes`, `err_nd_third_size`, `err_nd_zero_third_size`, `err_nd_fourth_size`, `err_nd_size_vector`, `err_nd_reshape` |
| 13 | `dot_matrices`, `err_dot_sizes`, `err_dot_vector_length`, `err_dot_dim_orientation` |
| 14 | `any_ignores_nan` |
| 15 | `isvector_empty` |
| 16 | `toc_after_bare_tic`, `err_toc_before_tic`, `err_toc_value_before_tic`, `err_toc_after_handle_tic` |
| 17 | `size_vectors_constructors`, `size_vectors_reshape_repmat`, `err_reshape_placeholder_divisible`, `err_reshape_two_placeholders`, `err_size_vector_column` |
| 18 | `linspace_floor_count` |
| 19 | `reduction_all_option`, `err_reduction_char_dim`, `err_cumsum_all`, `err_max_char_dim`, `err_size_char` |
| 20 | Unit tests: the size parser and `fmt_dim` in `src/builtins/args.rs`, `eps` in `src/builtins/core.rs`, `round` in `src/builtins/math.rs`, the scaled norm in `src/builtins/linalg.rs` |

## Status

Done (2026-09-27). All twenty acceptance tests are covered: 56 golden cases
(21 script cases and 35 `err_*`) in `tests/cases/01c-builtin-arguments/`,
and 39 new unit tests in `src/builtins/args.rs`, `core.rs`, `linalg.rs` and
`math.rs` and in `src/interp.rs`. The registry holds 80 builtins, with `e`
gone. The N-D row of Known bugs stays open, narrowed to an honest error.
