# 14b — N-D functions

## Goal

The builtins taught N-D. Cycle 14 made the N-D array a value: its
constructors, indexing, operators and display, with every other builtin
refusing it through one gate. This cycle moves the everyday library past
the gate: the reductions along any dimension, the element-wise math,
`squeeze`, `permute` and `cat`, N-D bracket concatenation, `repmat`, and
MAT-files that hold N-D arrays. Everything still not on `ND_OK` keeps
refusing an N-D argument by name.

After this module a user can do things that were not possible before; every
bullet in Scope must be demonstrable by at least one acceptance test below.

## Scope

- **The reductions along any dimension:** `sum`, `prod`, `mean`, `any`,
  `all`, `max`, `min`, `cumsum` and `cumprod` take an N-D array, by the
  MathWorks `sum` page's rules, which each of them follows:
  - with no dimension, a reduction "operates along the first array
    dimension whose size does not equal 1" (dimension 1 when every size is
    1), so `sum(ones(1, 1, 3))` is 3; today's special cases of an empty
    0x0 argument are kept
  - along `dim`, "the size of S in this dimension becomes 1 while the sizes
    of all other dimensions remain the same as in A" (`cumsum` and
    `cumprod` keep every size), trailing sizes of 1 dropped
  - along `dim` past `ndims(A)`, "sum returns A when dim is greater than
    ndims(A)", and likewise every reduction here returns what it returns
    today for a matrix and a dimension past 2, values and storage alike, so a
    `-0` stays `-0` and every 2-D answer is unchanged (`1/sum(-0, 3)` is
    `-Inf`, `prod(complex(1, 0), 3)` keeps its complex storage); along a
    dimension of size 1 within `ndims(A)`, each result element is the
    reduction of its one element, as for a matrix today (so `sum(-0, 1)` is
    `+0`, as it is today, which the Known deviations table records against
    the page's "or when size(A,dim) is 1"); `any` and `all` give, for each
    element, what they give for a vector of that one element, a `NaN`
    ignored as today; the index output of `max` and `min` is all 1s
  - along a dimension of size 0 each result element reduces nothing, as for
    a matrix today (`sum` 0, `prod` 1, `mean` `NaN`, `any` false, `all`
    true), and `max` and `min` give an empty result along it
  - `'all'` reduces every element, as today; `max(A, [], dim)` and
    `min(A, [], dim)` take any dimension and give the index along it as a
    second output; `max(A, B)` and `min(A, B)` of two arrays broadcast
    across every dimension, and two arrays with a dimension, `max(A, B,
    dim)`, are refused with `max takes two arrays or one array and a
    dimension, not both.` (and `min` alike), where today the second array
    is dropped in silence; `NaN` handling, complex rules and the classes of
    the results are today's
- **The element-wise math keeps every dimension:** `abs`, `sqrt`, `exp`,
  `log`, `log2`, `log10`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`,
  `sinh`, `cosh`, `tanh`, `floor`, `ceil`, `round` (with or without a digit
  count), `fix`, `sign`, `isnan`, `isinf`, `isfinite`, `real`, `imag`,
  `conj` and `angle` of an N-D array are N-D arrays of its shape, real and
  complex as today; the two-argument `mod`, `rem`, `atan2`, `hypot`,
  `power` and `complex` broadcast across every dimension, with cycle 14's
  operand-size message naming every dimension
- **`squeeze`** (new), by the MathWorks page: "B = squeeze(A) returns an
  array with the same elements as the input array A, but with dimensions of
  length 1 removed"; "If A is a row vector, column vector, scalar, or an
  array with no dimensions of length 1, then squeeze returns the input A";
  a result with fewer than two dimensions left is a column, as "a
  1-by-1-by-3 array" becomes "a 3-by-1 column vector". Class and complex
  storage are kept; a cell or struct argument is refused as a builtin
  refuses one today
- **`permute`** (new), by the MathWorks page: "the ith dimension of the
  output array is the dimension dimorder(i) from the input array".
  `dimorder` is a row of positive integers that holds each of 1 to n exactly
  once, with n at least `ndims(A)`; a dimension past `ndims(A)` is of size
  1. Anything else is `permute's dimension order must hold each of 1 to n
  once, with n at least ndims(A).` Class and complex storage are kept, and
  `permute(M, [2 1])` of a matrix is its plain transpose
- **`cat`** (new): `cat(dim, A1, ..., An)` joins its arrays along `dim`, a
  positive integer, which may be past every argument's `ndims`
  (`cat(3, A, B)` of two matrices makes pages). Every other dimension must
  agree, dimensions past an array's `ndims` being 1, else today's
  `Dimensions of arrays being concatenated are not consistent.`; by the
  MathWorks page, "When concatenating an empty array to a nonempty array,
  cat omits the empty array in the output", and when every input is empty
  the result is the empty the sizes give. The result's class follows the
  bracket rule (char if any is char, logical if all are logical, double
  otherwise), complex if any input is. `cat(dim)` with no arrays is `[]`,
  and with one array is that array
- **Brackets join N-D arrays:** with an N-D operand, `[A, B]` joins along
  dimension 2 and `[A; B]` along dimension 1 by `cat`'s rule, so `[A, A]`
  of a 2x3x4 is 2x6x4, and `cell2mat` of N-D elements likewise. With no N-D
  operand a bracket keeps today's 2-D rule exactly, its empties, shapes and
  classes included (`[1:0]` is 0x0 and `[zeros(1, 0), zeros(0, 1)]` 0x0,
  where `cat` of the same is an error by its page's rule). Cycle 14's
  `Concatenation of N-D arrays is not supported.` is retired; a mismatch is
  today's inconsistent-dimensions message
- **`repmat`** takes N-D: `repmat(A, m, n, p, ...)` and `repmat(A, [m n p
  ...])` tile any array, an N-D one included, along every dimension given,
  every size judged by `check_dims` before allocating; trailing counts of 1
  dropped as the constructors drop them
- **MAT-files hold N-D arrays:** `save` writes an N-D numeric, logical or
  char array, in a variable, a cell or a field, with its full dimensions
  array, and `load` reads one back with every dimension, the MAT reader's
  existing bounds applied to every dimension (each checked against the
  bytes there, the whole shape by `check_dims`, before allocating).
  `save -ascii` of an N-D array keeps cycle 14's refusal, `N-D arrays are
  not supported by 'save'.`, since the text format has rows alone. A
  dimension a version 5 MAT-file cannot hold, past 2,147,483,647, is
  refused before anything is written, `Unable to save variable '<var>': a
  dimension past 2147483647 cannot be written in a MAT-file of version
  5.`, where today it is written wrapped. A cell or
  struct array of more than two dimensions in a file is still refused, with
  the reader's existing text
- **At most 1,048,576 dimensions.** Every shape is judged by
  `check_dims`, which also refuses more than 2^20 dimensions with `Arrays
  have at most 1048576 dimensions.`, and `cat` judges its `dim` against the
  same bound before it builds a shape, so no small argument can make a list
  of dimensions that costs gigabytes (`cat(2^28, 1, 2)` would)
- **`ND_OK` grows by exactly the builtins above:** `sum`, `prod`, `mean`,
  `any`, `all`, `max`, `min`, `cumsum`, `cumprod`, the element-wise math
  listed, `squeeze`, `permute`, `cat` and `repmat`. Every other builtin
  keeps the gate's refusal (`sort`, `find`, `diff`, `num2str`, `fliplr`,
  the linear algebra, the strings, the sets, the plots and the rest)
- **The rows and cases this retires.** The Known bugs row "Constructors take
  two sizes only" narrows to `cell`, which stays 2-D. The cases that pinned
  a refusal this cycle lifts are removed, each named in the commit with the
  reason: `01c-builtin-arguments/err_nd_size_vector` (`repmat`), and from
  cycle 14 `err_gate_sum`, `err_gate_feval_abs`, `err_gate_second_argument`
  (the gate's coverage moves to builtins still gated), `err_hcat_nd`,
  `err_vcat_nd`, `err_save_nd` and `err_save_nd_workspace`. One case
  outside those is changed, not retired: `11-strings-and-io/err_load_mat_faults`
  built a 1x1x2 double to pin the reader's refusal of more than two
  dimensions, which this cycle lifts for numeric arrays; that file becomes a
  1x1x2 cell, which the reader still refuses with the same text, so the
  case's expected output is unchanged

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.
- Every builtin not named above: `sort`, `find`, `diff`, `median`, `std`,
  `var`, `mode`, `fliplr`, `flipud`, `circshift`, `ipermute`, `horzcat`,
  `vertcat`, `num2str`, `mat2str`, `arrayfun`, `cellfun` over N-D inputs,
  and every linear-algebra, numerics, string, set and plotting builtin.
- N-D cell and struct arrays.

## Design notes

Recorded as decisions are made. Start from the
"Key designs to preserve" section of docs/ARCHITECTURE.md and record here:
which files and types changed, which invariants were preserved (column-major
storage, 1-based to 0-based conversion at the index boundary, the `end` stack,
name resolution order, the output sink), and any deviation from this spec
that was accepted deliberately.

Settled at planning:

- **One rule for every reduction.** The `sum` page's three sentences fix the
  default dimension, the result's size and the case of a dimension of size
  1; every reduction here follows them, so a reduction along dimension `d`
  of any array is one kernel over the array seen as `[before, size(d),
  after]`, three numbers, whatever its `ndims`.
- **Brackets and `cat` share one kernel for N-D operands,** so they cannot disagree there, and the
  empty rule the `cat` page gives, an empty beside a nonempty omitted, is
  the one brackets already follow for a matrix (`[zeros(0, 5); ones(2, 3)]`
  is 2x3 today); when every operand is empty, a bracket of matrices keeps
  today's answer, which `cat`'s rule would change (settled in testing,
  below).
- **The retired cases,** because each pinned a refusal this cycle replaces
  with the working feature; the gate itself stays pinned by cases on
  builtins that remain gated.

Settled in testing, before the code that follows it:

- **Past `ndims`, today's answer.** The first build computed a reduction
  past `ndims` element by element, which turned a `-0` into `+0` and
  dropped a complex storage `prod` kept, changing 2-D answers; every
  reduction now returns there what it returns today.
- **Brackets of matrices keep today's rule.** Routing every bracket through
  `cat`'s rule changed 2-D answers when every operand was empty (`[1:0]`
  became 1x0, `[zeros(1, 0), zeros(0, 1)]` an error); only a bracket with an
  N-D operand now takes `cat`'s rule.
- **`max(A, B, dim)` is refused,** since dropping `B` in silence gave a wrong
  answer, now reachable with N-D arguments.
- **A MAT dimension past 2^31 - 1 is refused** before anything is written,
  where the writer wrapped it; and **at most 2^20 dimensions**, so `cat`'s
  scalar `dim` cannot build a list of dimensions that costs gigabytes.

Recorded during implementation:

- **Files and types changed.** `src/value.rs`: `along_dim`, the
  three-number view `[before, n, after]` of any shape along one dimension,
  which the reductions, the scans, `cat` and the brackets of N-D operands,
  and `repmat` share. `src/builtins/math.rs`: `default_dim`,
  `reduced_dims`, `judged`, `is_zero_by_zero`, `past_ndims` and
  `each_slice`, the one kernel, are new; `reduce`, `reduce_c`, `scan`,
  `extremum_along`, `extremum_along_arg` and `extremum_index` run through
  them, handing the argument back past `ndims` as they always did for a
  matrix and a dimension past 2; `extremum_value` refuses two arrays and
  a dimension; and `abs` of a complex array keeps every dimension.
  `src/builtins/complex.rs`: `real`, `imag`, `angle` and `complex` keep
  every dimension (`conj` already did). `src/builtins/linalg.rs`:
  `squeeze`, `permute` (with `permute_order` and `permuted`) and `cat` are
  new, and `repmat` reads `args::shape_dims` and tiles through `tile`.
  `src/interp.rs`: `concat` is `cat`'s kernel, which `hcat` and `vcat`
  call when an operand is N-D, each keeping its 2-D rule otherwise;
  `refuse_nd_concat` and the special case of a bracket of one N-D array
  are gone. `src/builtins/mat.rs`: the writer writes every dimension, one
  `int32` each, refusing one an `int32` cannot hold, and the reader takes
  them. `src/error.rs`: `permute_order`, `extremum_two_arrays_and_dim`,
  `save_dim_too_large` and `too_many_dims` are new and `nd_concatenation`
  is retired. `src/builtins/mod.rs`: `ND_OK` gains the 46 names Scope
  lists, the registry 254 builtins. `src/builtins/numerics.rs`:
  `first_dim` is `math::default_dim`, and `reduce_along` keeps its own
  rule for a dimension past `ndims`, each element reduced on its own,
  and is `reduce` within `ndims`. `src/builtins/args.rs`: `MAX_NDIMS`,
  2^20, is new and `check_dims` refuses a longer shape; the comments
  record `shape` now serving `eye` and `cell` alone.
- **Invariants preserved.** Column-major storage: element `(b, k, a)` of
  the three-number view is `b + before * (k + n * a)`, the column-major
  offset whatever the array's `ndims`, and `permute` walks its argument by
  the column-major strides. One-based to zero-based: indexing is untouched;
  a dimension argument and `permute`'s order are sizes and dimension
  numbers, not subscripts, and are read as `d - 1` where they are used, as
  `sum(A, d)` always was. The `end` stack, name resolution and the output
  sink are untouched. Invariant 6: every shape computed here, a
  reduction's, a concatenation's, `repmat`'s whole shape and a MAT-file's,
  goes through `check_dims` before anything is allocated; every loop runs
  over the elements produced or read, never over a dimension of an empty
  array; `permute`, like broadcasting, steps only the dimensions past 1; no
  `unwrap` or `expect` was added on anything a program controls, and no
  width or precision reaches a formatter.
- **Every 2-D answer is the one it was.** A 2-D matrix along dimension 1
  is the view `before = 1`, each slice a contiguous column, and along 2
  the view `after = 1`, each slice a row read in column order, which is
  how `reduce` read them before; `sum0`, the product and the running scans
  accumulate in the same order, so no digit moves. Past `ndims` no kernel
  runs: `reduce`, `reduce_c` and `scan` hand the argument back
  (`past_ndims`), the path a matrix and a dimension past 2 always took, so
  `1/sum(-0, 3)`, `1/mean(-0, 3)` and `1/cumsum(-0, 3)` are `-Inf`,
  `prod(complex(1, 0), 3)` keeps its complex storage while `sum` of it,
  whose two parts are reduced apart and stored by the flag rule, is real,
  and `any` and `all` convert the argument to a logical, so `any(NaN, 3)`
  is still `NaN's cannot be converted to logicals.`; the index of `max`
  and `min` there is all 1s, and an N-D array past its own `ndims` takes
  the same path. `numerics::reduce_along` keeps its own rule past `ndims`,
  each element reduced on its own, which makes `var(X, 0, 3)` zeros.
  Along a dimension of size 1 within `ndims` each element is reduced on
  its own, as a matrix's always was: `sum(-0, 1)`, `mean(-0, 1)` and
  `cumsum(-0, 1)` are `+0`, and `any` and `all` give each element their
  answer for a one-element vector, `any(NaN, 1)` false; both are Known
  deviations rows, verify first, the spec's earlier gloss of `A ~= 0`
  there being exact for every element but `NaN`. In testing, every
  reduction of a table of 2-D arguments (`-0`, `NaN`, `Inf`, complex
  arguments with zero imaginary parts, logicals, chars and empties) along
  dimensions 1 to 5, with no dimension and with `'all'`, gave the cycle
  14 build's answer in value, sign, class, shape and storage, errors
  included, and so did `median`, `std`, `var`, `mode` and `dot`. Not
  verified against MATLAB.
- **When every array of a concatenation is empty.** In `cat` and in a
  bracket with an N-D operand, the empties are joined by the agreement
  rule, the 2-D 0x0s among them left out as `[]` always is, and the result
  is a 0x0 of the class only when nothing is left: so `cat(3, zeros(2, 0),
  zeros(2, 0))` is 2x0x2, as the spec's rule gives, and `cat(2, 1:0)` is
  1x0. A bracket of 2-D operands alone keeps its own rule, in `hcat` and
  `vcat` as before: every empty is left out and a 0x0 of the class stands
  when nothing is left, so `[1:0]`, `[zeros(1, 0), zeros(0, 1)]`,
  `[zeros(0, 3); zeros(0, 2)]` and `cell2mat({zeros(3, 0), zeros(2, 0)})`
  are 0x0 and `[zeros(1, 0); true]` is a logical, as they were. In
  testing, every bracket form and `cell2mat` of a grid of 2-D operand
  pairs (empties of every shape, logicals, chars and complex values) gave
  the cycle 14 build's answer. `[[] []]`, `['' '']` and `cat(3)` are `[]`
  of their class.
- **A dimension past every array's.** `cat(dim, ...)` with `dim` past
  every argument's `ndims` gives the result `dim` dimensions whenever
  more than one array is joined along it, so `dim` is judged against
  `args::MAX_NDIMS`, 2^20, before the result's list of sizes is made:
  `cat(2^21, 1, 2)`, `cat(1e10, 1, 2)` and `cat(1e300, 1, 2)` are `Arrays
  have at most 1048576 dimensions.`, never a request for that many sizes,
  while `cat(2^20, 1, 2)` has 1048576 dimensions and `cat(1e10, A)` is
  `A`, since one array joined along it makes no dimension. The reductions
  and `max` and `min` never grow a shape, so a dimension of `1e300` costs
  nothing there.
- **At most 2^20 dimensions.** `check_dims` refuses a shape of more than
  `args::MAX_NDIMS` dimensions before it judges the sizes, `Arrays have at
  most 1048576 dimensions.`, so the size message never names millions of
  sizes; every builder of a shape already goes through it, a
  constructor's size vector, a reshape, a broadcast, the subscripts of a
  read and a growth, a reduction's, a concatenation's, `repmat`'s and a
  MAT-file's dimensions array, which is judged as the file writes it. A
  constructor drops trailing sizes of 1 first, so `zeros([2 ones(1,
  2^21)])` is 2x1. No case or test goes past the bound: cycle 14's use at
  most about 40,002 dimensions and this cycle's cases at most a million,
  `permute(A, 1:1e6)`, beside the unit tests at the bound itself.
- **`max(A, B, dim)`.** `extremum_value` read the dimension of a
  three-argument call and ignored its second argument, so `max([1 5], [3
  4], 2)` was `5`, a wrong answer; with a second argument that has
  elements it is now `max takes two arrays or one array and a dimension,
  not both.` (and `min` alike), for a matrix and an N-D array, with one
  output or two. An empty second argument, the placeholder `[]` or any
  other empty, is the one-array form, as before, since it holds nothing
  that could be dropped.
- **A MAT dimension past 2^31 - 1.** The writer wrote each dimension as
  `k as i32`, which wrapped a dimension past 2,147,483,647, one only an
  empty array can have (`zeros(0, 1, 5e9)`), into a file that reads back
  as another shape. `mat::matrix` now refuses such a dimension first,
  `Unable to save variable '<var>': a dimension past 2147483647 cannot be
  written in a MAT-file of version 5.`, naming the variable wherever the
  array sits, a cell element or a field included; `mat::write` builds the
  whole file in memory before `save` writes any of it, so no file is made
  or changed. 2,147,483,647 itself is written and read.
- **`cat` takes arrays.** A cell, a struct, a handle or an `MException`
  after the dimension is refused as every numeric builtin refuses one,
  `This operation is not supported for a value of class 'cell'.`; MATLAB's
  `cat` joins cells too, which is recorded in Known deviations. `cat` of
  one array goes through the kernel, so it is stored by the flag rule, as
  a bracket of one is.
- **The complex gate.** `squeeze`, `permute` and `cat` keep complex
  storage, which the spec requires, so they are on `TAKES_COMPLEX` beside
  `ND_OK`; `squeeze` and `permute` keep an all-zero imaginary part, since
  no element changes, while `cat` stores by the flag rule, as the brackets
  do. `repmat`, `max` and `min` stay off it, as today. A complex order to
  `permute` is its own refusal, since the gate lets `permute` through.
- **`permute`'s order.** Anything that is not a real row of positive
  integers holding each of 1 to n once, with n at least `ndims(A)`, is the
  one message, a column, an N-D row, a char, a cell and a complex order
  included, judged before the order's length sizes anything.
- **`repmat`.** The counts come through `args::shape_dims`, so `repmat(A,
  n)` is n-by-n and a size vector is a row, as before. The result's size
  along each dimension is the argument's times its count, trailing sizes
  of 1 dropped before `check_dims` names them, so `repmat(A, 1e5, 1e5)` of
  a 2x3x4 is refused as `200000x300000x4`. An empty result returns at
  once; otherwise the array is tiled one dimension at a time, each step a
  `cat` of copies along that dimension, and since every count is at least
  1 no step is larger than the result.
- **MAT-files.** The reader takes the dimensions of a char or a numeric
  class (logicals included) whatever their number; any other class of more
  than two dimensions keeps `an array in it has more than two dimensions`,
  a sparse array and an object included, so a file refused before is
  refused with the same text unless it holds an N-D array this cycle
  reads. A dimension is written as an `int32`, as a matrix's two always
  were.
- **A ninth case pinned a refusal this cycle lifts.** Beside the eight the
  spec names, `11-strings-and-io/err_load_mat_faults` built, among its
  faulty files, a 1x1x2 double with no data and expected `an array in it
  has more than two dimensions`. Its dimensions are legal now, so the same
  file would be `its data is corrupt.`, the reader's answer for data that
  does not hold its shape. The case is changed, as Scope's last bullet
  records: its fourth file's class is now a cell, `flags(1)` for
  `flags(6)`, a 1x1x2 cell, which the reader still refuses with the same
  text, so its expected output is unchanged; nothing else in it moved.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/14b-nd-functions/`, as a `.m` case unless it says otherwise.
Expected output follows from this spec's rules and the existing display.
`A` is `reshape(1:24, 2, 3, 4)` throughout, so `A(i, j, k)` is
`i + 2*(j - 1) + 6*(k - 1)`.

1. `sum(A, 3)` → `[40 48 56; 44 52 60]`; `S = sum(A); disp(size(S)); disp(S(:)')` → `1 3 4` and `3 7 11 15 19 23 27 31 35 39 43 47`; `sum(ones(1, 1, 3))` → 3; `size(sum(A, 5))` → `2 3 4`; `sum(A, 'all')` → 300; `size(sum(zeros(2, 0, 3), 2))` → `2 1 3` and every element 0. Every 2-D answer past `ndims` unchanged: `1/sum(-0, 3)`, `1/mean(-0, 3)` and `1/cumsum(-0, 3)` → `-Inf`, `1/sum(-0, 1)` → `Inf`, `isreal(prod(complex(1, 0), 3))` → 0 and `isreal(sum(complex(1, 0), 3))` → 1, as today. Cases: `sum_nd.m`, `reduce_nd_empty_dim.m`, `reduce_nd_nan.m`, `reduce_nd_complex.m`, `nd_functions_empty_huge.m`, `nd_functions_many_dims.m`, `reduce_2d_past_ndims_unchanged.m`.
2. `mean(A, 3)` → `[10 12 14; 11 13 15]`; `prod(2 * ones(2, 2, 2), 3)` → all 4; `any(A > 22, 3)` → `[0 0 1; 0 0 1]` (class logical); `all(A > 0, 3)` → all 1; `any(A, 5)` → `A ~= 0`, `size` `2 3 4`. Cases: `reduce_nd_mean_prod_any_all.m`, `reduce_nd_empty_dim.m`, `reduce_nd_nan.m`, `reduce_nd_complex.m`, `nd_functions_empty_huge.m`.
3. `max(A, [], 3)` → `[19 21 23; 20 22 24]` and its index output all 4; `M = min(A, [], 2); disp(size(M)); disp(M(:)')` → `2 1 4` and `1 2 7 8 13 14 19 20`; `max(A)` along the first dimension, `size` `1 3 4`; `max(A, 12)` broadcast, checked through its `(:)'` row; `[m, i] = max(A, [], 5)` → `m` is `A` and `i` all 1. `err_*` cases: `max([1 5], [3 4], 2)` and `min(A, A, 3)` → `max takes two arrays or one array and a dimension, not both.` and its `min` form. Cases: `max_min_nd.m`, `err_max_nd_sizes.m`, `err_max_complex_nd.m`, `reduce_nd_nan.m`, `nd_functions_empty_huge.m`, `nd_functions_many_dims.m`, `err_max_two_arrays_dim.m`, `err_min_two_arrays_dim.m`.
4. `C = cumsum(A, 3)` → `C(:, :, 4)` equal to `sum(A, 3)` and `size(C)` `2 3 4`; `cumprod(2 * ones(1, 1, 3))` → pages 2, 4, 8. Cases: `cumsum_cumprod_nd.m`, `reduce_nd_nan.m`, `reduce_nd_complex.m`, `nd_functions_empty_huge.m`.
5. The element-wise math on N-D, each checked by shape and by its `(:)'` row: `abs(-A)`, `sqrt(A .^ 2)` equal to `A` (self-checked), `floor(A / 5)`, `mod(A, 5)`, `rem(-A, 5)`, `round(A / 7)`, `isnan(A ./ 0 - Inf)`, `sign(A - 12)`; `atan2(ones(1, 1, 2), ones(1, 1, 2))` and `hypot` broadcast against a 2x1; a complex N-D array through `real`, `imag`, `conj` and `abs`. Cases: `elementwise_math_nd.m`, `elementwise_math_nd_every_function.m`, `complex_math_nd.m`, `err_mod_nd_sizes.m`, `nd_functions_empty_huge.m`.
6. `squeeze`: `size(squeeze(ones(1, 1, 3)))` → `3 1`; of `zeros(2, 1, 3)` → `2 3`; of `zeros(1, 3, 1, 2)` → `3 2`; of `zeros(1, 1, 1, 4)` → `4 1`; of `ones(2, 3)` → `2 3`; of `ones(1, 5)` → `1 5`; `squeeze` keeps class (logical) and complex storage. Cases: `squeeze_nd.m`, `err_squeeze_cell.m`, `err_squeeze_struct.m`, `nd_functions_empty_huge.m`.
7. `permute`: `P = permute(A, [3 1 2])` → `size` `4 2 3`, `P(4, 2, 3)` 24, `P(1, 1, 2)` 3; `size(permute(ones(2, 3), [2 1]))` → `3 2`, equal to the transpose; `size(permute(ones(2, 3), [3 1 2]))` → `1 2 3`; `permute(A, [1 2 3 4])` equal to `A`; `permute('ab', [2 1])` a 2x1 char. Cases: `permute_nd.m`, `nd_functions_empty_huge.m`, `nd_functions_many_dims.m`.
8. `err_*` cases: `permute(A, [1 2])` and `permute(A, [1 1 2])` → the permute message. Cases: `err_permute_order_short.m`, `err_permute_order_repeat.m`, `err_permute_order_gap.m`, `err_permute_order_fraction.m`, `err_permute_order_column.m`, `err_permute_order_char.m`.
9. `cat`: `cat(3, [1 2; 3 4], [5 6; 7 8])` → 2x2x2, displayed with both pages; `size(cat(1, A, A))` → `4 3 4`; `size(cat(2, ones(2, 2), ones(2, 3)))` → `2 5`; `size(cat(3, ones(2, 2), []))` → `2 2`; `size(cat(4, 1, 2))` → `1 1 1 2`; `class(cat(3, 'ab', 'cd'))` → `char`; `cat(3)` → `[]`. `err_*`: `cat(2^21, 1, 2)` → `Arrays have at most 1048576 dimensions.` Cases: `cat_nd.m`, `err_cat_dim_zero.m`, `nd_functions_empty_huge.m`, `nd_functions_many_dims.m`, `err_cat_too_many_dims.m`.
10. `err_*`: `cat(3, ones(2, 2), ones(2, 3))` → `Dimensions of arrays being concatenated are not consistent.` Cases: `err_cat_dims_inconsistent.m`, `err_cat_nd_dims_inconsistent.m`.
11. Brackets: `size([A, A])` → `2 6 4`; `size([A; A])` → `4 3 4`; `B = [A, A]; disp(B(1, 4, 1))` → 1; an `err_*` case `[A, ones(2, 3)]` → the inconsistent-dimensions message. With no N-D operand, today's answers: `size([1:0])`, `size([zeros(1, 0)])`, `size([zeros(1, 0), zeros(0, 1)])`, `size([zeros(0, 3); zeros(0, 2)])`, `size([zeros(3, 0), zeros(3, 0), 1:0])` and `size(cell2mat({zeros(3, 0), zeros(2, 0)}))` → `0 0` each, and `class([zeros(1, 0); true])` → `logical`. Cases: `brackets_nd.m`, `err_hcat_nd_inconsistent.m`, `err_vcat_nd_inconsistent.m`, `nd_functions_empty_huge.m`, `brackets_2d_empties_unchanged.m`.
12. `repmat`: `size(repmat(A, 1, 1, 2))` → `2 3 8`; `size(repmat([1 2], [2 1 3]))` → `2 2 3`; `size(repmat(A, 2, 1))` → `4 3 4`; `R = repmat([1 2], 1, 1, 2); disp(R(:)')` → `1 2 1 2`; an `err_*` case `repmat(1, 1e5, 1e5, 1e5)` → the size message naming every size. Cases: `repmat_nd.m`, `err_repmat_size_every_dim.m`, `nd_functions_empty_huge.m`, `nd_functions_many_dims.m`.
13. MAT-files: `A`, a logical `L = A > 12`, a char `c = 'ab'; c(:, :, 2) = 'cd'`, a cell `q = {A}` and a struct field `s.f = A` saved to `scratch_nd.mat`, cleared, loaded back, and each checked with `isequal` (or its `size` and `(:)'` row for the cell and the field) and `class`; the file deleted at the end. Cases: `mat_nd_round_trip.m`, `mat_nd_workspace.m`.
14. `err_*`: `save('scratch_nd.txt', 'A', '-ascii')` of an N-D `A` → `N-D arrays are not supported by 'save'.`, with no file left behind. `C = zeros(0, 1, 5e9); save('scratch_big.mat', 'C')` → `Unable to save variable 'C': a dimension past 2147483647 cannot be written in a MAT-file of version 5.`, with no file left behind. Cases: `err_save_ascii_nd.m`, `err_save_mat_dim_too_large.m`.
15. The gate still holds, each an `err_*` case: `sort(zeros(2, 2, 2))` → `N-D arrays are not supported by 'sort'.`; `feval(@find, zeros(2, 2, 2))` → by `'find'`; `diff(zeros(2, 2, 2))` → by `'diff'`. Cases: `err_gate_sort_nd.m`, `err_gate_feval_find_nd.m`, `err_gate_diff_nd.m`, `err_gate_second_argument_nd.m`.
16. Unit tests: the default dimension and the three-number view of a reduction; a dimension of size 1 and past `ndims`; `permute`'s index arithmetic and its refusals; `cat`'s agreement rule and the empty rule; `repmat`'s shape judged before allocating; the MAT writer's and reader's N-D dimensions array, and the reader's bound on a hostile dimensions array. Tests: `the_default_dimension_is_the_first_whose_size_is_not_one`, `a_reduction_runs_over_the_three_number_view`, `a_dimension_of_size_one_or_past_ndims_reduces_each_element`, `max_and_min_reduce_and_index_along_any_dimension`, `the_element_wise_math_keeps_every_dimension`, `permute_moves_each_element_to_its_permuted_subscript`, `permute_refuses_every_other_order`, `cat_joins_along_any_dimension_when_the_others_agree`, `squeeze_removes_the_dimensions_of_length_one`, `repmat_tiles_every_dimension_and_judges_the_shape_first`, `an_nd_array_is_written_and_read_with_every_dimension`, `a_hostile_dimensions_array_is_a_clean_error`, `save_writes_an_nd_array_and_ascii_refuses_one`, `the_gate_refuses_an_nd_argument_to_every_other_builtin`, `operators_and_for_see_every_dimension`.
17. Every existing case passes unchanged, apart from the eight cases this spec removes and the one fixture it changes. Cases: the whole golden suite.

## Status

Done (2026-09-30)
