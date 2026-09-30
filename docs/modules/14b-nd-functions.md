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
  - along a dimension of size 1, `dim` past `ndims(A)` included, each result
    element is the reduction of one element: "sum returns A when dim is
    greater than ndims(A) or when size(A,dim) is 1", and likewise `prod`,
    `mean`, `cumsum`, `cumprod`, `max` and `min` return `A` (as a double,
    as today's class rules give), `any` and `all` return `A ~= 0`, and the
    index output of `max` and `min` is all 1s
  - along a dimension of size 0 each result element reduces nothing, as for
    a matrix today (`sum` 0, `prod` 1, `mean` `NaN`, `any` false, `all`
    true), and `max` and `min` give an empty result along it
  - `'all'` reduces every element, as today; `max(A, [], dim)` and
    `min(A, [], dim)` take any dimension and give the index along it as a
    second output; `max(A, B)` and `min(A, B)` of two arrays broadcast
    across every dimension; `NaN` handling, complex rules and the classes of
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
- **Brackets join N-D arrays:** `[A, B]` joins along dimension 2 and
  `[A; B]` along dimension 1 by `cat`'s rule, so `[A, A]` of a 2x3x4 is
  2x6x4. Cycle 14's `Concatenation of N-D arrays is not supported.` is
  retired; a mismatch is today's inconsistent-dimensions message
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
  not supported by 'save'.`, since the text format has rows alone. A cell or
  struct array of more than two dimensions in a file is still refused, with
  the reader's existing text
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
  `err_vcat_nd`, `err_save_nd` and `err_save_nd_workspace`

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
- **Brackets and `cat` share one kernel,** so they cannot disagree, and the
  empty rule the `cat` page gives is the one brackets already follow for a
  matrix (`[zeros(0, 5); ones(2, 3)]` is 2x3 today).
- **The retired cases,** because each pinned a refusal this cycle replaces
  with the working feature; the gate itself stays pinned by cases on
  builtins that remain gated.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/14b-nd-functions/`, as a `.m` case unless it says otherwise.
Expected output follows from this spec's rules and the existing display.
`A` is `reshape(1:24, 2, 3, 4)` throughout, so `A(i, j, k)` is
`i + 2*(j - 1) + 6*(k - 1)`.

1. `sum(A, 3)` → `[40 48 56; 44 52 60]`; `S = sum(A); disp(size(S)); disp(S(:)')` → `1 3 4` and `3 7 11 15 19 23 27 31 35 39 43 47`; `sum(ones(1, 1, 3))` → 3; `size(sum(A, 5))` → `2 3 4`; `sum(A, 'all')` → 300; `size(sum(zeros(2, 0, 3), 2))` → `2 1 3` and every element 0.
2. `mean(A, 3)` → `[10 12 14; 11 13 15]`; `prod(2 * ones(2, 2, 2), 3)` → all 4; `any(A > 22, 3)` → `[0 0 1; 0 0 1]` (class logical); `all(A > 0, 3)` → all 1; `any(A, 5)` → `A ~= 0`, `size` `2 3 4`.
3. `max(A, [], 3)` → `[19 21 23; 20 22 24]` and its index output all 4; `M = min(A, [], 2); disp(size(M)); disp(M(:)')` → `2 1 4` and `1 2 7 8 13 14 19 20`; `max(A)` along the first dimension, `size` `1 3 4`; `max(A, 12)` broadcast, checked through its `(:)'` row; `[m, i] = max(A, [], 5)` → `m` is `A` and `i` all 1.
4. `C = cumsum(A, 3)` → `C(:, :, 4)` equal to `sum(A, 3)` and `size(C)` `2 3 4`; `cumprod(2 * ones(1, 1, 3))` → pages 2, 4, 8.
5. The element-wise math on N-D, each checked by shape and by its `(:)'` row: `abs(-A)`, `sqrt(A .^ 2)` equal to `A` (self-checked), `floor(A / 5)`, `mod(A, 5)`, `rem(-A, 5)`, `round(A / 7)`, `isnan(A ./ 0 - Inf)`, `sign(A - 12)`; `atan2(ones(1, 1, 2), ones(1, 1, 2))` and `hypot` broadcast against a 2x1; a complex N-D array through `real`, `imag`, `conj` and `abs`.
6. `squeeze`: `size(squeeze(ones(1, 1, 3)))` → `3 1`; of `zeros(2, 1, 3)` → `2 3`; of `zeros(1, 3, 1, 2)` → `3 2`; of `zeros(1, 1, 1, 4)` → `4 1`; of `ones(2, 3)` → `2 3`; of `ones(1, 5)` → `1 5`; `squeeze` keeps class (logical) and complex storage.
7. `permute`: `P = permute(A, [3 1 2])` → `size` `4 2 3`, `P(4, 2, 3)` 24, `P(1, 1, 2)` 3; `size(permute(ones(2, 3), [2 1]))` → `3 2`, equal to the transpose; `size(permute(ones(2, 3), [3 1 2]))` → `1 2 3`; `permute(A, [1 2 3 4])` equal to `A`; `permute('ab', [2 1])` a 2x1 char.
8. `err_*` cases: `permute(A, [1 2])` and `permute(A, [1 1 2])` → the permute message.
9. `cat`: `cat(3, [1 2; 3 4], [5 6; 7 8])` → 2x2x2, displayed with both pages; `size(cat(1, A, A))` → `4 3 4`; `size(cat(2, ones(2, 2), ones(2, 3)))` → `2 5`; `size(cat(3, ones(2, 2), []))` → `2 2`; `size(cat(4, 1, 2))` → `1 1 1 2`; `class(cat(3, 'ab', 'cd'))` → `char`; `cat(3)` → `[]`.
10. `err_*`: `cat(3, ones(2, 2), ones(2, 3))` → `Dimensions of arrays being concatenated are not consistent.`
11. Brackets: `size([A, A])` → `2 6 4`; `size([A; A])` → `4 3 4`; `B = [A, A]; disp(B(1, 4, 1))` → 1; an `err_*` case `[A, ones(2, 3)]` → the inconsistent-dimensions message.
12. `repmat`: `size(repmat(A, 1, 1, 2))` → `2 3 8`; `size(repmat([1 2], [2 1 3]))` → `2 2 3`; `size(repmat(A, 2, 1))` → `4 3 4`; `R = repmat([1 2], 1, 1, 2); disp(R(:)')` → `1 2 1 2`; an `err_*` case `repmat(1, 1e5, 1e5, 1e5)` → the size message naming every size.
13. MAT-files: `A`, a logical `L = A > 12`, a char `c = 'ab'; c(:, :, 2) = 'cd'`, a cell `q = {A}` and a struct field `s.f = A` saved to `scratch_nd.mat`, cleared, loaded back, and each checked with `isequal` (or its `size` and `(:)'` row for the cell and the field) and `class`; the file deleted at the end.
14. `err_*`: `save('scratch_nd.txt', 'A', '-ascii')` of an N-D `A` → `N-D arrays are not supported by 'save'.`, with no file left behind.
15. The gate still holds, each an `err_*` case: `sort(zeros(2, 2, 2))` → `N-D arrays are not supported by 'sort'.`; `feval(@find, zeros(2, 2, 2))` → by `'find'`; `diff(zeros(2, 2, 2))` → by `'diff'`.
16. Unit tests: the default dimension and the three-number view of a reduction; a dimension of size 1 and past `ndims`; `permute`'s index arithmetic and its refusals; `cat`'s agreement rule and the empty rule; `repmat`'s shape judged before allocating; the MAT writer's and reader's N-D dimensions array, and the reader's bound on a hostile dimensions array.
17. Every existing case passes unchanged, apart from the eight cases this spec removes.

## Status

Planned
