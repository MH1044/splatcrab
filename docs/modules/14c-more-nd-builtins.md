# 14c — More N-D builtins

## Goal

The rest of the everyday library taught N-D. Cycle 14 made the N-D array
a value behind a gate, and cycle 14b moved the reductions, the
element-wise math and the rearrangements past it. This cycle moves the
search, sort and statistics functions and the flips past the gate, adds
the N-D rearrangements and index conversions MATLAB programs reach for
(`flip`, `circshift`, `ipermute`, `horzcat`, `vertcat`, `sub2ind` and
`ind2sub`), and pins the N-D behaviours no golden case pinned yet. Every
builtin still not on `ND_OK` keeps refusing an N-D argument by name.

After this module a user can do things that were not possible before; every
bullet in Scope must be demonstrable by at least one acceptance test below.

## Sources

Quoted at planning, on 2026-09-30, from the MathWorks reference pages.

- **S1, `sort`.** "If `A` is a multidimensional array, then `sort(A)`
  operates along the first array dimension whose size does not equal 1,
  treating the elements as vectors." On `dim`: "If no value is specified,
  then the default is the first array dimension whose size does not equal
  1." "`sort` returns `A` if `dim` is greater than `ndims(A)`." "The `sort`
  function uses a stable sorting algorithm." "`I` is the same size as `A`."
- **S2, `find`.** "If `X` is a vector, then `find` returns a vector with the
  same orientation as `X`." "If `X` is a multidimensional array, then
  `find` returns a column vector of the linear indices of the result." "If
  `X` is a multidimensional array with `N > 2`, then `col` is a linear
  index over the `N-1` trailing dimensions of `X`. This preserves the
  relation `X(row(i),col(i)) == v(i)`." "`k` is an empty row vector or
  empty column vector when `X` is an empty array or has no nonzero
  elements." "`find` uses the convention that `k` is an empty matrix `[]`
  when `X` is an empty matrix `[]` or a scalar zero."
- **S3, `diff`.** "By default, `diff` operates along the first array
  dimension whose size does not equal 1." "`Y = diff(X,n,dim)` is the nth
  difference calculated along the dimension specified by `dim`." On `n`:
  "When `n` is larger than the dimension being operated on, `diff` returns
  an empty array."
- **S4, `median`.** "If `A` is a multidimensional array, then `median(A)`
  treats the values along the first array dimension whose size does not
  equal 1 as vectors." "The size of `M` in this dimension becomes 1, while
  the sizes of all other dimensions remain the same as in `A`." "`median`
  returns `A` when `dim` is greater than `ndims(A)`."
- **S5, `std` and `var`.** "If `A` is a multidimensional array, then
  `std(A)` operates along the first array dimension whose size does not
  equal 1, treating the elements as vectors. The size of `S` in this
  dimension becomes `1`, while the sizes of all other dimensions are the
  same as in `A`." "If `dim` is greater than `ndims(A)`, then `std(A)`
  returns an array of zeros the same size as `A`." `var`'s page says the
  same of `V` and "`var(A)` returns an array of zeros the same size as
  `A`".
- **S6, `mode`.** "If `A` is a multidimensional array, then `mode(A)`
  treats the values along the first array dimension whose size does not
  equal `1` as vectors and returns an array of most frequent values", "The
  size of this dimension becomes `1` while the sizes of all other
  dimensions remain the same". "`F` is the same size as `M`". "`mode`
  returns `A` if `dim` is greater than `ndims(A)`."
- **S7, `flip`.** "`B = flip(A)` returns array `B` the same size as `A`,
  but with the order of the elements reversed." "`B = flip(A,dim)`
  reverses the order of the elements in `A` along dimension `dim`." "If
  `A` is vector, then `flip(A)` reverses the order of the elements along
  the length of the vector." "If `A` is a matrix, then `flip(A)` reverses
  the elements in each column." "If `A` is an N-D array, then `flip(A)`
  operates on the first dimension of `A` in which the size value is not
  1."
- **S8, `fliplr` and `flipud`.** "For multidimensional arrays, `fliplr`
  operates on the planes formed by the first and second dimensions." The
  same sentence of `flipud`, and: "The operation flips the elements on each
  page independently."
- **S9, `circshift`.** "`Y = circshift(A,K)` circularly shifts the elements
  in array `A` by `K` positions." "If `K` is an integer, then `circshift`
  shifts along the first dimension of `A` whose size does not equal 1."
  "If `K` is a vector of integers, then each element of `K` indicates the
  shift amount in the corresponding dimension of `A`." "`Y =
  circshift(A,K,dim)` circularly shifts the values in array `A` by `K`
  positions along dimension `dim`." "Inputs `K` and `dim` must be
  scalars." "Positive `K` shifts toward the end of the dimension and
  negative `K` shifts toward the beginning." "If the shift amount is
  greater than the length of the corresponding dimension in `A`, then the
  shift circularly wraps."
- **S10, `ipermute`.** "`A = ipermute(B,dimorder)` rearranges the
  dimensions of an array `B` in the order specified by the vector
  `dimorder` such that `B = permute(A,dimorder)`. In other words, the
  `i`th dimension of the input array becomes the dimension `dimorder(i)`
  in the output array." `dimorder`: "specified as a row vector with unique,
  positive integer elements representing the dimensions of the input
  array."
- **S11, `horzcat` and `vertcat`.** "`C = horzcat(A,B)` concatenates `B`
  horizontally to the end of `A` when `A` and `B` have compatible sizes
  (the lengths of the dimensions match except in the second dimension)."
  "When concatenating an empty array to a nonempty array, `horzcat` omits
  the empty array in the output." "If all input arguments are empty and
  have compatible sizes, then `horzcat` returns an empty array whose size
  is equal to the output size as when the inputs are nonempty." `vertcat`
  alike along the first dimension.
- **S12, `sub2ind`.** "`ind = sub2ind(sz,I1,I2,...,In)` returns linear
  indices for multidimensional subscripts". `sz`: "Size of array,
  specified as a vector of positive integers." `I1,...,In`: "can be arrays
  of the same size, or any of them can be scalar." `ind`: "If the
  subscript inputs all have the same size, then `ind` is also that size.
  If the subscript inputs are a mix of scalars and arrays, then `ind` has
  the size of the nonscalar subscript inputs." Algorithms: "For an array
  `A`, if `ind = sub2ind(size(A),I1,…,In)`, then `A(ind(k)) =
  A(I1(k),…,In(k))` for all `k`." Examples: `sub2ind([3 3],[1 2 3 1],[2 2
  2 3])` is `4 5 6 7`; `sub2ind([2 2 2],[1 2 1 2],[2 2 1 1],[1 1 2 2])` is
  `3 4 5 6`; `sub2ind(size(A),2,1,2)` of a 3x4x2 `A` is `14`.
- **S13, `ind2sub`.** "`[I1,I2,...,In] = ind2sub(sz,ind)` returns `n`
  arrays `I1,I2,...,In` containing equivalent multidimensional subscripts
  corresponding to linear indices `ind`". "The size of each array
  `I1,I2,…,In` is the same as the size of the input `ind`." With fewer
  outputs: "If you specify only two output arguments, `ind2sub` ignores the
  third dimension of the array and returns subscripts for a 2-dimensional
  array with size 2-by-4 instead", and with one, "returns subscripts for a
  1-dimensional array with size 1-by-8", its example giving `1:8` for
  `ind2sub([2 2 2], 1:8)`. Outside the size: "`ind2sub` does not produce an
  error for this operation. Instead, `ind2sub` automatically adds columns
  and expands the matrix size to 3-by-5", `[row,col] = ind2sub([3 1],[9 11
  13 14])` giving `row` `3 2 1 2` and `col` `3 4 5 5`.
- **S14, `arrayfun`.** "The arrays `A` and `B` have the same size." "The
  arrays `A1,...,An` all must have the same size." With `UniformOutput`
  false: "`arrayfun` returns the outputs of `func` in cell arrays."
- **S15, `num2str` and `mat2str`.** `num2str`: `A` "specified as a numeric
  array", with no statement of how an N-D array is laid out. `mat2str`:
  "converts the numeric or logical matrix `X`", `X` "specified as a numeric
  or logical matrix".

## Scope

- **`sort` takes an N-D array** (S1): `sort(A)`, `sort(A, dim)`,
  `sort(A, direction)` and `sort(A, dim, direction)` sort each slice along
  the first dimension whose size is not 1, or along `dim`, stably, `NaN`
  last ascending and first descending, as a column is sorted today;
  `[B, I] = sort(...)` gives `I` the size of `A`, each element's position
  along the dimension; along a dimension past `ndims(A)` `B` is `A` and `I`
  is all 1s. The class is kept and a complex argument refused, as today
- **`find` takes an N-D array** (S2): `k = find(X)` of an N-D `X` is a
  column of linear indices, 0x1 when no element is nonzero, a vector keeping
  its orientation as today; `find(X, n)` and `find(X, n, 'last')` as today;
  `[row, col] = find(X)` of an N-D `X` gives `col` as the linear index over
  the dimensions past the first, so `X(row(i), col(i))` is the `i`th
  nonzero element, and `[row, col, v] = find(X)` adds the values in `X`'s
  class, every output a column. By S2's convention a scalar zero, `find(0)`
  and `find(false)`, gives `[]`, 0x0, for every output, where today it is
  1x0; every other 2-D answer is unchanged
- **`diff` takes an N-D array** (S3): `diff(X)` and `diff(X, n)` work each
  of the `n` rounds along the first dimension whose size is not 1 of what
  the round before left, as today; `diff(X, n, dim)` works all `n` rounds
  along `dim`, the result having `max(size(X, dim) - n, 0)` elements there
  and every other dimension kept, so along a dimension past `ndims(X)` it
  is empty there: `size(diff(ones(2, 3), 1, 3))` is `2 3 0`, where today it
  is `N-D arrays are not supported.` `diff(X, 0, dim)` is `X`, whatever
  `dim`. A `dim` that would give the result more than 2^20 dimensions is
  judged against that bound before any list of sizes is made, `Arrays have
  at most 1048576 dimensions.`, so `diff(1:3, 1, 1e10)` costs nothing. The
  result is a double, as today
- **`median`, `std`, `var` and `mode` take an N-D array** (S4 to S6):
  along the first dimension whose size is not 1, or along `dim`, the size
  there becoming 1 and every other kept, each slice reduced as a column is
  today (`median` of a slice holding a `NaN` is `NaN`, `mode` ignores a
  `NaN`, `std` and `var` normalise by `N - 1` or by `N` as `w` says, the
  smallest of a tie is `mode`'s), `[M, F] = mode(...)` giving `F` the size
  of `M`; a slice with no elements gives `NaN` (and `F` 0), and a 0x0
  argument keeps today's answers. Along a dimension past `ndims(A)`,
  `median` and `mode` return `A` (`F` 1 for each element, 0 for a `NaN`),
  as they do today for a matrix, and `std` and `var` return zeros the size
  of `A` by S5, a `NaN` or `Inf` element included, where today `var(NaN,
  0, 3)` is `NaN`
- **`fliplr` and `flipud` take an N-D array** (S8): each page is flipped on
  its own, `fliplr` reversing the order along dimension 2 and `flipud`
  along dimension 1. The class is kept; a complex argument and a cell keep
  today's refusals
- **`flip`** (new, S7): `flip(A)` reverses the order of the elements along
  the first dimension whose size is not 1, and `flip(A, dim)` along `dim`,
  a positive integer read as the reductions read one; along a dimension
  past `ndims(A)` it returns `A`. Any numeric, logical or char array, its
  class kept; a complex argument is refused as `fliplr` refuses one, and a
  cell or struct as a numeric builtin refuses one
- **`circshift`** (new, S9): `circshift(A, K)` with an integer `K` shifts
  the elements along the first dimension whose size is not 1; with a vector
  `K`, element `i` of `K` shifts dimension `i`, an element past `ndims(A)`
  shifting a dimension of size 1, which moves nothing; `circshift(A, K,
  dim)` shifts along `dim` by the integer `K`. A positive shift moves
  elements toward the end of the dimension and a negative one toward its
  beginning, wrapping around, so a shift is taken modulo the size of its
  dimension and any integer a double holds costs one pass over the array;
  along a dimension of size 0 nothing moves, and an empty array is
  returned as it is.
  The class is kept; a complex argument and a cell are refused as by
  `flip`. `K` must be a nonempty real row or column of integers,
  `circshift's shift must be an integer or a vector of integers.`, and a
  single integer when `dim` is given, `circshift's shift must be one
  integer when a dimension is given.`; `dim` is read as the reductions read
  one
- **`ipermute`** (new, S10): `ipermute(B, dimorder)` is the array `A` for
  which `permute(A, dimorder)` is `B`, dimension `dimorder(i)` of the
  result being dimension `i` of `B`. `dimorder` must be what `permute`
  takes, a real row holding each of 1 to n once with n at least `ndims(B)`,
  else `ipermute's dimension order must hold each of 1 to n once, with n
  at least ndims(A).`; the class and complex storage are kept, as
  `permute` keeps them
- **`horzcat` and `vertcat`** (new, S11): `horzcat(A1, ..., An)` is
  `cat(2, A1, ..., An)` and `vertcat(A1, ..., An)` is `cat(1, A1, ...,
  An)`, since S11's rules are `cat`'s: an empty array beside a nonempty one
  is omitted, and when every input is empty the result is the empty array
  the sizes give, so `size(horzcat(zeros(1, 0), zeros(1, 0)))` is `1 0`,
  while the bracket `[zeros(1, 0), zeros(1, 0)]` keeps its 2-D rule and is
  0x0. With no argument each is `[]`. They take arrays only, as `cat`
  does, a cell refused with `This operation is not supported for a value
  of class 'cell'.`, and follow `cat`'s class, complex and
  inconsistent-dimensions rules
- **`sub2ind`** (new, S12): `sub2ind(sz, I1, ..., In)` with `sz` a real
  row or column of positive integers, else `sub2ind's size must be a vector
  of positive integers.`, and `n` at least 1 subscript arrays, all the same
  size or scalars, else `sub2ind's subscripts must have the same size, or
  be scalars.`, gives, as a double of the size of the subscripts that are
  not scalars (1x1 when all are), the linear index of `A(I1(k), ...,
  In(k))` in an array `A` of size `sz`, by S12's relation to indexing: with
  fewer subscripts than sizes the last subscript runs over the product of
  the remaining sizes, as an index with fewer subscripts folds the trailing
  dimensions, and the sizes past the end of `sz` are 1. A subscript that is
  not a positive integer within its dimension's size (the folded size for
  the last) is `sub2ind's subscripts must be positive integers within the
  size.`
- **`ind2sub`** (new, S13): `[I1, ..., Ik] = ind2sub(sz, ind)`, `k` the
  number of outputs asked for and 1 when none is, gives `k` doubles each
  the size of `ind`, reading the size as `k` dimensions: `sz` with 1s
  appended up to `k` sizes, and with more than `k` sizes, those from the
  `k`th on folded into one, so one output is `ind` itself and two outputs
  of `[2 2 2]` read it as 2x4. The last of the `k` dimensions has no bound,
  as S13's example shows (`[row, col] = ind2sub([3 1], [9 11 13 14])` gives
  `col` as `3 4 5 5`), so any positive integer converts. `sz` is judged as
  `sub2ind` judges it, `ind2sub's size must be a vector of positive
  integers.`, and an index that is not a positive integer is `ind2sub's
  indices must be positive integers.`
- **`arrayfun` takes N-D arrays** (S14): with every array argument of one
  size, an N-D size included, the result has that size, each element `f` of
  the elements in its position, called in column-major order through
  `Interp::call_nested` as today; arguments of different sizes keep today's
  `All of the input arguments must be of the same size and shape.`. With
  `'UniformOutput', false` an N-D argument is refused, `N-D arrays are not
  supported.`, since the cell array it would return would be N-D, which
  SplatCrab's cells never are
- **The N-D behaviours no case pinned, pinned:** `cell2mat` of N-D elements
  joins them by the bracket rule (`size(cell2mat({ones(2, 2, 2), ones(2,
  1, 2)}))` is `2 3 2`); `max` and `min` of an N-D logical keep the class
  logical; `[m, i] = max(A, [], 'all')` of an N-D array gives the linear
  index of the maximum; a shape of exactly 2^20 dimensions is made
  (`ndims(zeros([ones(1, 2^20 - 1) 2]))` is 1048576) and one more is
  refused
- **A folded size is named as asked:** where a read through fewer
  subscripts than dimensions folds sizes whose product passes what a
  `usize` holds, the refusal names the product as asked: `x = zeros(0,
  2^40, 2^40); y = x(:, :)` is `Requested 0x1.20893e+24 array exceeds the
  maximum array size.`, where today the product saturates and the message
  names `1.84467e+19`
- **`ND_OK` grows by exactly** `sort`, `find`, `diff`, `median`, `std`,
  `var`, `mode`, `fliplr`, `flipud`, `arrayfun`, `flip`, `circshift`,
  `ipermute`, `horzcat`, `vertcat`, `sub2ind` and `ind2sub`, and
  `TAKES_COMPLEX` by `ipermute`, `horzcat` and `vertcat`, which keep complex
  storage as `permute` and `cat` do. `num2str` (S15 gives no N-D layout) and
  `mat2str` (S15 takes a matrix) stay behind the gate, with every other
  builtin not named. The registry holds 261 builtins
- **The cases this retires.** Cycle 14b's `err_gate_sort_nd`,
  `err_gate_feval_find_nd` and `err_gate_diff_nd` pinned the gate on
  builtins this cycle teaches; they are removed, each named in the commit,
  and the gate stays pinned by cases on `num2str`, `mat2str` and a `feval`
  of a builtin still gated. In `docs/ARCHITECTURE.md` the N-D Known
  deviations row names the builtins still behind the gate, and records
  `horzcat` and `vertcat` refusing cells beside `cat`, and the changes of
  `find(0)` and of `std` and `var` past `ndims` are stated where the
  reductions and the search functions are described

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.
- N-D cells and structs, and so `cell(2, 3, 4)`, `arrayfun` with
  `UniformOutput` false over an N-D argument, and `mode`'s third output.
- `num2str` and `mat2str` of an N-D array, which stay behind the gate.
- Complex arguments to `sort`, `flip`, `circshift`, `fliplr`, `flipud`,
  `diff` and the statistics, which keep today's refusal; cells and structs
  given to `flip`, `circshift`, `fliplr`, `flipud`, `horzcat` and
  `vertcat`, which the pages allow and which stay refused.
- The class of `median`'s result, which S4 says is `A`'s and which stays a
  double, as today.

## Design notes

Recorded as decisions are made. Start from the
"Key designs to preserve" section of docs/ARCHITECTURE.md and record here:
which files and types changed, which invariants were preserved (column-major
storage, 1-based to 0-based conversion at the index boundary, the `end` stack,
name resolution order, the output sink), and any deviation from this spec
that was accepted deliberately.

Settled at planning:

- **One view for every slice.** `sort`, `diff`, `median`, `std`, `var`,
  `mode`, `flip` and `circshift` work along one dimension of any array, the
  three-number view `[before, n, after]` of cycle 14b (`Matrix::along_dim`)
  giving each slice; a 2-D matrix along dimension 1 or 2 is the view whose
  slices are its columns or its rows, read in the order they are read
  today, so every 2-D answer stays what it was.
- **`horzcat` and `vertcat` are `cat`**, not the brackets: S11's empty rule
  is `cat`'s, which differs from the brackets' 2-D rule only when every
  operand is empty.
- **`sub2ind` follows indexing** (S12's Algorithms sentence), so its fold of
  trailing sizes and its refusal of a subscript past its dimension are the
  index pipeline's, while `ind2sub`, whose page makes the last dimension
  unbounded, refuses only an index that is not a positive integer.
- **The retired cases,** because each pinned the gate's refusal on a
  builtin this cycle teaches; the gate itself stays pinned on builtins that
  remain behind it.
- **`find(0)` and the zeros of `std` and `var` past `ndims`** change 2-D
  answers because S2 and S5 state them; every other 2-D answer of the
  builtins taught here stays byte-identical.

Recorded during implementation:

- **Files and types changed.** `src/builtins/linalg.rs`: `sort` sees its
  argument through `value::along_dim` and `math::default_dim`, one slice
  of `[before, n, after]` at a time, and its index output takes every
  dimension; `find` gives an N-D argument a column and a scalar zero `[]`;
  `flipped`, the one reversal along a dimension, serves `fliplr`, `flipud`
  and the new `flip`; `circshift` and `shift_amounts`, `ipermute` and
  `inverse_order`, `horzcat` and `vertcat` over `joined` (which calls
  `interp::concat`, `cat`'s kernel), and `sub2ind`, `ind2sub` and
  `index_sizes` are new; `permute_order` takes the name its refusal
  gives. `src/builtins/numerics.rs`: `extent` reads every dimension;
  `map_slices` runs over the three-number view of any array along any
  dimension; `diff` reaches it along any dimension, its rounds along one
  dimension taken together; `deviation` is the rule `std` and `var` take
  past `ndims`, while `reduce_along` keeps its rule for `median` and
  `mode`. `src/builtins/cells.rs`: `map_elements`
  runs over every element of `arrayfun`'s arrays, gives a uniform result
  their dimensions, and refuses an N-D array with `'UniformOutput',
  false`. `src/interp.rs`: `fold_asked`, the fold of trailing dimensions
  as asked in `f64`, which `resolve_read` counts a colon by and
  `resolve_write` judges its grown shape by. `src/error.rs`:
  `dimension_order` (which `permute_order` is, for `permute`),
  `circshift_shift`, `circshift_shift_with_dim`, `index_size_vector`,
  `sub2ind_subscript_sizes`, `sub2ind_out_of_range` and `ind2sub_index`.
  `src/builtins/mod.rs`: `ND_OK` gains the seventeen names, `TAKES_COMPLEX`
  `ipermute`, `horzcat` and `vertcat`, and the registry holds 261
  builtins. `src/builtins/core.rs`: `arrayfun`'s comment.
- **Invariants preserved.** Column-major storage: every slice function
  reads element `k` of slice `(b, a)` at `b + before * (k + n * a)`, and
  `ind2sub`'s subscripts are the column-major remainders, so `A(I1, I2,
  I3)` is `A(ind)`. One-based to zero-based: indexing is untouched; a
  dimension, a shift, a size and the subscripts and indices of `sub2ind`
  and `ind2sub` are values, not subscripts, read one-based and turned
  into offsets where they are used, as `sum(A, d)` always was. The `end`
  stack, name resolution and the output sink are untouched; `arrayfun`
  still calls through `Interp::call_nested`. Invariant 6: `map_slices`
  judges its shape by `check_dims` before allocating, and a `dim` past
  `ndims` that would give the result more dimensions than
  `args::MAX_NDIMS` is refused before its list of sizes is made, so
  `diff(1:3, 1, 1e10)` is the refusal at once and `diff(X, 0, 1e300)` is
  `X`; the statistics' shapes go through the reductions' kernel and
  `horzcat`'s and `vertcat`'s through `cat`'s; `sort`, the flips,
  `circshift`, `ipermute`, `sub2ind`, `ind2sub` and `arrayfun` make no
  shape larger than an argument's. An empty argument returns at once in
  each of them, whatever its sizes, and none walks a dimension of size 1
  per element: `sort`, the flips and `circshift` pass over one without a
  loop, and the view costs one product over the dimensions, so
  `zeros([ones(1, 1e6) 2])` is sorted, flipped, shifted and found in time
  linear in its dimensions and its two elements, never their product. `circshift` takes each shift modulo its
  dimension's size with `rem_euclid` in `f64`, exact for any integer a
  double holds, and rotates each block of `before * n` elements once per
  dimension that moves; an entry of a vector shift past `ndims` sizes
  nothing. `sub2ind` judges and weighs each scalar subscript once, and
  `ind2sub` writes each output once, so both are linear in their inputs
  and outputs. `diff(X, n)` takes its consecutive rounds along one
  dimension together, each slice going through the same subtractions in
  the same order as round by round, so the shape of an array of many
  singleton dimensions is judged once per dimension that shrinks, not once
  per round. No `unwrap` or `expect` was added on anything a program
  controls, and no width reaches a formatter.
- **Every other 2-D answer is the one it was.** A matrix along 1 is the
  view `before = 1` and along 2 the view `after = 1`, each slice read in
  the order the columns and the transposed rows always were, so every
  slice function gets the same elements in the same order and no digit
  moves. `map_slices` still judges a matrix along its rows as the
  transpose it used to work through, `out` by `rows`, so a refusal names
  its sizes in the order it always did (`trapz(zeros(2^29, 0), 2)` is
  `Requested 1x536870912 array exceeds the maximum array size.`, as
  before). In testing, 4,126 forms over a table of 39 2-D arguments
  (`-0`, `NaN`, `Inf`, logicals, chars, empties of every orientation and
  empties with a dimension of a million, vectors, matrices and a matrix of
  `1e300`s) of `sort`, `find`, `diff`, `median`, `mode`, `std`, `var`,
  `fliplr`, `flipud`, `arrayfun`, `cellfun`, `trapz`, `cumtrapz`, `filter`,
  `permute`, `cat`, `cell2mat` and indexing, with every output, class,
  shape, storage and error text, gave the cycle 14b build's answer but
  for the changes Scope names: `find` of a scalar zero `0`, `-0` or
  `false` (24 forms, now 0x0 for every output), `std` and `var` past
  `ndims` of an argument holding a `NaN` or an `Inf` (28 forms, now
  zeros), `diff` along a dimension past `ndims` (117 forms: 78 empty
  results where the N-D refusal stood, and 39 a `dim` of `1e10` now
  `Arrays have at most 1048576 dimensions.`), and the folded size (2
  forms, below). In testing the comparison was run again over 9,840
  forms and 12,615 lines of output, the same builtins and `max`, `min`,
  `num2str` and `mat2str` over 40 arguments with every dimension from 1
  to 4 and `1e10`, and differed in the same four classes alone.
- **The folded size, for a write too.** The fold is the index pipeline's
  one fold, which a write through fewer subscripts than dimensions judges
  as a read does, so `resolve_write` judges its grown shape from
  `fold_asked` as well: `x = zeros(0, 2^40, 2^40); x(1:0, :) = 5` is
  `Requested 0x1.20893e+24 array exceeds the maximum array size.`, where
  it named `1.84467e+19` as the read did. Wherever the product fits a
  `usize`, `fold_asked` is exactly `fold_dims`'s, so no other read,
  write, bound or growth decision moves. Two consequences, both clean
  refusals: a fold whose product passes what a double holds is named
  `Inf`, `zeros([0 repmat(2^60, 1, 18)])` read through `x(:, :)` giving
  `Requested 0xInf array exceeds the maximum array size.`; and a write
  whose subscript lies within the fold but past what a `usize` holds, `x(1:0,
  2e19) = 5` of `zeros(0, 2^40, 2^40)`, is that size refusal naming the
  fold, where it was the ambiguous-growth error.
- **`find` of a scalar zero with a count.** S2's convention is about `X`,
  so `find(0, n)` and `find(0, n, 'last')` are `[]` too, for every
  output, where they were 1x0; a scalar that is not zero, `NaN`
  included, is found as before.
- **The order of the refusals.** `circshift` judges its shift first, then
  its dimension, then the shift's count beside it, so
  `circshift(1:3, 1.5, 2)` is the shift message and `circshift(1:3, [1
  1], 0)` the dimension's. `sub2ind` judges the size vector, then the
  subscripts' sizes, then their values, the scalars first and the arrays
  in argument order, so a scalar subscript past its size is refused even
  beside an empty array. `arrayfun` judges its arrays' sizes before it
  refuses an N-D array with `'UniformOutput', false`.
- **What the size and shift arguments take.** A char is refused as a
  shift, a size vector and a dimension order whatever its codes, as
  `permute` refuses one, and a cell or a struct gives the argument's own
  refusal; a logical is read by its values, so `circshift(A, true)` shifts
  by 1. The subscripts of `sub2ind` and the indices of `ind2sub` are read
  by value too, a char by its codes and a logical as 0 and 1, and an
  infinity is no integer. A complex shift, size, subscript or index, zero
  imaginary parts included, is refused by the registry's complex gate
  before `circshift`, `sub2ind` or `ind2sub` runs, since none of them is on
  `TAKES_COMPLEX`, as `flip` refuses a complex array; `ipermute`, which is
  on it to keep complex storage, refuses a complex order with its own
  message.

Settled in testing:

- **`ind2sub`'s outputs are judged together.** A program can ask for as
  many outputs as the targets it writes, or builds and evaluates: `eval`
  of `[a, a, ..., b] = ind2sub(...)` with a hundred thousand targets asks
  for a hundred thousand. Each output is an array the size of `ind`, so
  before any is written the outputs are judged as one result by
  `check_shape`, their elements `numel(ind)` by `k` and their lists of
  sizes `ndims(ind)` by `k`, and no count of outputs makes `ind2sub` hold
  more than one array may: a hundred thousand outputs of `1:1e5` are
  `Requested 100000x100000 array exceeds the maximum array size.`, where
  they would have asked for 10^10 doubles. Outputs that fit are written
  as Scope says; an index of more than 2^27 elements asked for two
  outputs is refused too, since the two together pass that bound.
- **`arrayfun`'s outputs of an N-D array, likewise.** Each uniform output
  carries the arrays' list of sizes, so for an N-D array those lists are
  judged together, `ndims` by the outputs, before `f` is called: a
  thousand outputs over an array of 2^20 dimensions are refused at once.
  A matrix's two sizes are never judged, so no 2-D call moves.
- **A subscript of 1 adds nothing to `sub2ind`'s index.** The strides are
  products of the sizes in `f64`, and past what a double holds a stride is
  `Inf`; `0 * Inf` would make the index `NaN`, so a subscript of 1 is
  never weighed: `sub2ind([1e300 1e300 2], 1, 1, 1)` is 1, and a
  subscript past 1 along such a stride gives `Inf`, the double the index
  rounds to.
- **Two refusals under one acceptance item.** Items 3, 8 and 11 each name
  two inputs for one refusal, and an `err_*` case stops at its first
  error, so the second has a case of its own: `err_diff_too_many_dims_huge`
  (`diff(1:3, 1, 1e10)`), `err_ipermute_order_repeat` (`ipermute(A, [1 1
  2])`) and `err_ind2sub_index_fraction` (`ind2sub([2 3], 1.5)`).
- **Each bound on outputs has its case:** `err_ind2sub_outputs_bounded`
  and `err_arrayfun_nd_outputs_bounded`, and `find_scalar_zero` pins the
  count forms of `find` of a scalar zero.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/14c-more-nd-builtins/`, as a `.m` case unless it says otherwise.
Expected output follows from this spec's rules and the existing display.
`A` is `reshape(1:24, 2, 3, 4)` throughout, so `A(i, j, k)` is
`i + 2*(j - 1) + 6*(k - 1)`.

1. `sort`: `B = sort(-A, 3)` → `B(:, :, 1)` is `[-19 -21 -23; -20 -22 -24]`; `[B, I] = sort(A, 3, 'descend')` → `I(:, :, 1)` all 4; `isequal(sort(A(:, :, [3 1 4 2]), 3), A)` → 1; `size(sort(A))` → `2 3 4`; `[~, I] = sort(ones(1, 1, 3)); disp(I(:)')` → `1 2 3` (stable, along dimension 3); `s = sort(cat(3, NaN, 1, 2)); disp(s(:)')` → `1 2 NaN`, and with `'descend'` → `NaN 2 1`; `[B, I] = sort(A, 5)` → `isequal(B, A)` 1 and `all(I(:) == 1)` 1; `class(sort(cat(3, 'b', 'a')))` → `char`. Cases: `sort_nd.m`, `sort_nd_stable_nan.m`.
2. `find`: `k = find(A > 22)` → `size(k)` `2 1`, `k'` `23 24`; `[r, c] = find(A == 14)` → `[r c]` `2 7`; `[r, c, v] = find(A .* (A > 22))` → `[r c v]` `[1 12 23; 2 12 24]`; `find(A > 20, 1)` → 21; `find(A, 2, 'last')'` → `23 24`; `size(find(zeros(2, 2, 2)))` → `0 1`; `size(find(ones(1, 3, 2)))` → `6 1`. A scalar zero: `size(find(0))` and `size(find(false))` → `0 0`, and `[r, c] = find(0)` → both `0 0`; unchanged: `size(find(zeros(1, 3)))` → `1 0`, `size(find([]))` → `0 0`, `size(find(zeros(2, 3)))` → `0 1`, `size(find(1))` → `1 1`. Cases: `find_nd.m`, `find_scalar_zero.m`.
3. `diff`: `D = diff(A, 1, 3)` → `size(D)` `2 3 3`, `D(:, :, 1)` all 6; `D = diff(A)` → `size(D)` `1 3 4`, every element 1; `size(diff(A, 2))` → `1 2 4`; `isequal(diff(A, 3, 3), zeros(2, 3))` → 1; `size(diff(A, 5, 3))` → `2 3 0`; `size(diff(ones(2, 3), 1, 3))` → `2 3 0`; `isequal(diff(A, 0, 7), A)` → 1. `err_*`: `diff(1:3, 1, 2^21)` → `Arrays have at most 1048576 dimensions.`, exit 1, and `diff(1:3, 1, 1e10)` alike, at once. Cases: `diff_nd.m`, `err_diff_too_many_dims.m`.
4. The statistics: `median(A, 3)` → `[10 12 14; 11 13 15]`; `M = median(A)` → `size(M)` `1 3 4` and `2 * M(:)'` → `3 7 11 15 19 23 27 31 35 39 43 47`; `S = std(A, 0, 3)` → `fprintf('%.4f', S(1, 1))` `7.7460` and every element equal; `var(A, 1, 3)` → all 45; `var(A, 0, 3)` → all 60; `X = cat(3, [1 2], [1 3], [2 3]); [M, F] = mode(X, 3)` → `M` `[1 3]`, `F` `[2 2]`; `M = median(zeros(2, 0, 3), 2)` → `size(M)` `2 1 3`, every element `NaN`; `size(mode(zeros(2, 0, 3)))` → `1 0 3`. Past `ndims`: `isequal(median(A, 4), A)`, `isequal(mode(A, 4), A)`, `isequal(std(A, 0, 4), zeros(2, 3, 4))` and `isequal(var(A, 1, 4), zeros(2, 3, 4))` → 1 each; `var(NaN, 0, 3)` → 0 and `std([NaN Inf], 0, 3)` → `[0 0]`; `[M, F] = mode([1 NaN], 3)` → `M` `[1 NaN]`, `F` `[1 0]`. Cases: `statistics_nd.m`, `statistics_past_ndims.m`.
5. `fliplr` and `flipud`: `F = fliplr(A)` → `F(:, :, 2)` `[11 9 7; 12 10 8]`; `U = flipud(A)` → `U(:, :, 4)` `[20 22 24; 19 21 23]`; `c = cat(3, 'ab', 'cd'); x = fliplr(c)` → `x(:, :, 2)` `dc`, class `char`; `size(fliplr(zeros(2, 0, 3)))` → `2 0 3`. Cases: `flip_lr_ud_nd.m`.
6. `flip`: `F = flip(A, 3)` → `F(:, :, 1)` `[19 21 23; 20 22 24]`; `isequal(flip(A), flipud(A))` → 1; `flip(1:3)` → `3 2 1`; `flip([1 2; 3 4])` → `[3 4; 1 2]`; `flip([1 2; 3 4], 2)` → `[2 1; 4 3]`; `isequal(flip(A, 5), A)` → 1; `x = flip(cat(3, 1, 2, 3)); disp(x(:)')` → `3 2 1`; `flip('abc')` → `cba`. `err_*`: `flip({1, 2})` → `This operation is not supported for a value of class 'cell'.`; `flip([1i 2])` → `Complex values are not supported by 'flip'.` Cases: `flip_nd.m`, `err_flip_cell.m`, `err_flip_complex.m`.
7. `circshift`: `circshift(1:5, 2)` → `4 5 1 2 3`; `circshift(1:5, -1)` → `2 3 4 5 1`; `circshift(1:5, 7)` → `4 5 1 2 3`; `circshift([1 2; 3 4], 1)` → `[3 4; 1 2]`; `circshift([1 2; 3 4], 1, 2)` → `[2 1; 4 3]`; `circshift([1 2; 3 4], [1 1])` → `[4 3; 2 1]`; `C = circshift(A, 1, 3)` → `C(:, :, 1)` `[19 21 23; 20 22 24]`; `isequal(circshift(A, [0 0 4]), A)`, `isequal(circshift(A, 1e15, 3), A)` and `isequal(circshift(A, [1 0 0 5]), circshift(A, 1))` → 1 each; `circshift('abc', 1)` → `cab`. Refusals, each printed through `try` and `catch`: `circshift(1:3, 1.5)`, `circshift(1:3, NaN)`, `circshift(1:3, [])` and `circshift(1:3, ones(2, 2))` → `circshift's shift must be an integer or a vector of integers.`; `circshift(1:3, [1 1], 2)` → `circshift's shift must be one integer when a dimension is given.`; and an `err_*` case for each message, exit 1. Cases: `circshift_nd.m`, `circshift_refusals.m`, `err_circshift_shift.m`, `err_circshift_shift_with_dim.m`.
8. `ipermute`: `M = [1 2 3; 4 5 6; 7 8 9; 10 11 12]; B = permute(M, [2 1])` → `isequal(ipermute(B, [2 1]), M)` 1; `P = permute(A, [3 1 2])` → `isequal(ipermute(P, [3 1 2]), A)` 1; `size(ipermute(ones(4, 2, 3), [3 1 2]))` → `2 3 4`; `isreal(ipermute(complex(ones(2, 3), 0), [2 1]))` → 0. `err_*`: `ipermute(A, [1 2])` and `ipermute(A, [1 1 2])` → `ipermute's dimension order must hold each of 1 to n once, with n at least ndims(A).` Cases: `ipermute_nd.m`, `err_ipermute_order.m`.
9. `horzcat` and `vertcat`: `size(horzcat(A, A))` → `2 6 4`; `size(vertcat(A, A))` → `4 3 4`; `horzcat([1 2], 3)` → `1 2 3`; `vertcat([1 2], [3 4])` → `[1 2; 3 4]`; `size(horzcat(zeros(1, 0), zeros(1, 0)))` → `1 0` beside `size([zeros(1, 0), zeros(1, 0)])` → `0 0`; `size(vertcat([1; 2], []))` → `2 1`; `size(horzcat())` → `0 0`; `horzcat('ab', 'cd')` → `abcd`. `err_*`: `horzcat({1}, {2})` → `This operation is not supported for a value of class 'cell'.`; `vertcat([1 2], [1 2 3])` → `Dimensions of arrays being concatenated are not consistent.` Cases: `horzcat_vertcat.m`, `err_horzcat_cell.m`, `err_vertcat_inconsistent.m`.
10. `sub2ind`: `sub2ind([3 3], [1 2 3 1], [2 2 2 3])` → `4 5 6 7`; `sub2ind([2 2 2], [1 2 1 2], [2 2 1 1], [1 1 2 2])` → `3 4 5 6`; `sub2ind([3 4 2], 2, 1, 2)` → 14; `sub2ind([3 4], [1; 2], 3)` → `[7; 8]`; `sub2ind([2 3 4], 2, 12)` → 24; `sub2ind([2 3], 1, 2, 1)` → 3; `i = sub2ind(size(A), [1 2], [3 1], [4 2]); disp(A(i))` → `23 8`; `size(sub2ind([2 3], zeros(1, 0), zeros(1, 0)))` → `1 0`. Refusals through `try` and `catch`: `sub2ind([2 3], 3, 1)`, `sub2ind([2 3], 1.5, 1)`, `sub2ind([2 3], 1, 4)` and `sub2ind([2 3 4], 1, 13)` → `sub2ind's subscripts must be positive integers within the size.`; `sub2ind([2 3], [1 2], [1 2 3])` → `sub2ind's subscripts must have the same size, or be scalars.`; `sub2ind([2 0], 1, 1)` → `sub2ind's size must be a vector of positive integers.`; and an `err_*` case for each message. Cases: `sub2ind_nd.m`, `sub2ind_refusals.m`, `err_sub2ind_out_of_range.m`, `err_sub2ind_sizes.m`, `err_sub2ind_size_vector.m`.
11. `ind2sub`: `[row, col] = ind2sub([3 3], [3 4 5 6])` → `row` `3 1 2 3`, `col` `1 2 2 2`; `[I1, I2, I3] = ind2sub([2 2 2], [3 4 5 6])` → `1 2 1 2`, `2 2 1 1`, `1 1 2 2`; `[row, col, page] = ind2sub([3 4 2], 14)` → `2 1 2`; `[row, col] = ind2sub([2 2 2], 1:8)` → `col` `1 1 2 2 3 3 4 4`; `row = ind2sub([2 2 2], 1:8)` → `1 2 3 4 5 6 7 8`; `[row, col] = ind2sub([3 1], [9 11 13 14])` → `row` `3 2 1 2`, `col` `3 4 5 5`; `[r, c, p] = ind2sub([2 3], 5)` → `1 3 1`; `[r, c, p] = ind2sub([2 3], 7)` → `1 1 2`; `[r, c] = ind2sub([2 3], [1 2; 3 4])` → `size(r)` `2 2`, `c` `[1 1; 2 2]`. `err_*`: `ind2sub([2 3], 0)` and `ind2sub([2 3], 1.5)` → `ind2sub's indices must be positive integers.`; `ind2sub([2 -3], 1)` → `ind2sub's size must be a vector of positive integers.` Cases: `ind2sub_nd.m`, `err_ind2sub_index.m`, `err_ind2sub_size_vector.m`.
12. `arrayfun`: `B = arrayfun(@(x) x * 2, A)` → `size(B)` `2 3 4`, `B(2, 3, 4)` 48; `isequal(arrayfun(@(x, y) x + y, A, A), 2 * A)` → 1. `err_*`: `arrayfun(@(x, y) x + y, A, ones(2, 3))` → `All of the input arguments must be of the same size and shape.`; `arrayfun(@(x) x, A, 'UniformOutput', false)` → `N-D arrays are not supported.` Cases: `arrayfun_nd.m`, `err_arrayfun_nd_sizes.m`, `err_arrayfun_nd_nonuniform.m`.
13. The pins: `size(cell2mat({ones(2, 2, 2), ones(2, 1, 2)}))` → `2 3 2`; `class(max(true(2, 2, 2), [], 3))` and `class(min(true(2, 2, 2)))` → `logical`; `[m, i] = max(A, [], 'all')` → `24 24`, `[m, i] = min(-A, [], 'all')` → `-24 24`; `ndims(zeros([ones(1, 2^20 - 1) 2]))` → 1048576. `err_*`: `zeros([ones(1, 2^20) 2])` → `Arrays have at most 1048576 dimensions.` Cases: `nd_pins.m`, `err_nd_one_dimension_too_many.m`.
14. `err_*`: `x = zeros(0, 2^40, 2^40); y = x(:, :)` → `Requested 0x1.20893e+24 array exceeds the maximum array size.` Cases: `err_folded_size_named.m`.
15. The gate still holds, each an `err_*` case: `num2str(A)` → `N-D arrays are not supported by 'num2str'.`; `mat2str(A)` → by `'mat2str'`; `feval(@kron, A, 1)` → by `'kron'`. Cases: `err_gate_num2str_nd.m`, `err_gate_mat2str_nd.m`, `err_gate_feval_kron_nd.m`.
16. Unit tests: the three-number view for each slice function, a 2-D matrix along dimensions 1 and 2 reading its columns and rows in today's order; `find`'s trailing-dimension column index; `diff`'s shape along and past `ndims`, and its dimension bound judged before a shape is built; the statistics past `ndims`; `flip`, `circshift` (a shift modulo the size, a vector shift, a huge shift) and `ipermute` (the inverse of `permute` over every order of four dimensions); `horzcat` and `vertcat` as `cat`; `sub2ind` and `ind2sub` against the index pipeline over every subscript of a 2x3x4 array, with fewer and more subscripts or outputs than dimensions; the folded size named as asked. Tests: `slices_along_any_dimension_read_a_matrix_as_today`, `find_gives_the_trailing_dimensions_as_one_column_index`, `diff_along_or_past_ndims_judges_the_shape_first`, `the_statistics_past_ndims_follow_their_pages`, `flip_reverses_along_one_dimension`, `circshift_wraps_modulo_the_size`, `ipermute_inverts_permute`, `horzcat_and_vertcat_are_cat`, `sub2ind_agrees_with_indexing`, `ind2sub_folds_and_leaves_the_last_dimension_unbounded`, `a_folded_size_is_named_as_asked`.
17. Every existing case passes unchanged, apart from the three cases this spec removes. Cases: the whole golden suite.

## Status

Done (2026-09-30)
