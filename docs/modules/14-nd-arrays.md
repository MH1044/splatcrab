# 14 — N-D arrays

## Goal

Numeric, logical and char arrays of any number of dimensions: made by the
constructors, by `reshape` and by indexed growth, indexed with any number of
subscripts, combined element-wise with broadcasting across every dimension,
and displayed page by page. Every builtin not yet taught N-D refuses an N-D
argument through one central gate, as cycle 10's complex gate refuses a
complex one, so no 2-D kernel ever reads the first page of an N-D array in
silence. This is the first of two cycles; 14b, nd-functions, teaches the
builtins (reductions along any dimension, the element-wise math, `squeeze`,
`permute`, `cat`, N-D concatenation, `repmat`, MAT-files).

After this module a user can do things that were not possible before; every
bullet in Scope must be demonstrable by at least one acceptance test below.

## Scope

- **The representation.** A `Matrix` keeps its `rows` and `cols` and gains
  the dimensions past the second, stored normalised: a trailing dimension of
  1 is never stored, so every 2-D matrix is exactly what it is today, and
  `ndims` is 2 plus the stored count. Storage stays column-major, generalised:
  element `(i1, i2, ..., ik)` sits at `i1 + d1*(i2 + d2*(i3 + ...))`, zero-based.
  `Value::dims` answers every dimension, so each caller is revisited; every
  caller either handles an N-D value or refuses it. Cells and structs stay
  2-D: a cell or struct array is never N-D in this cycle, though an element or
  a field may hold an N-D matrix
- **Sizes judged before allocating.** `args::check_dims`, beside
  `check_shape`, judges the product of any number of sizes in `f64` against
  `MAX_ELEMS` before anything is allocated, and refuses in `check_shape`'s
  words with every size: `Requested 100000x100000x100000 array exceeds the
  maximum array size.` Every N-D shape this cycle computes goes through it
- **The gate.** `builtins::ND_OK` names the builtins that take an N-D
  argument, and `nd_gate`, run by `Interp::call_builtin` beside
  `complex_gate`, refuses an N-D matrix argument to every other builtin with
  `N-D arrays are not supported by '<name>'.` In this cycle `ND_OK` is:
  `zeros`, `ones`, `rand`, `NaN`, `nan`, `Inf`, `inf`, `true`, `false`,
  `size`, `ndims` (new), `numel`, `length`, `isempty`, `isscalar`,
  `isvector`, `class`, `isa`, `islogical`, `ischar`, `isnumeric`, `iscell`,
  `isstruct`, `isreal`, `isequal`, `reshape`, `disp`, `double`, `logical`,
  `char`, `fprintf`, `sprintf`, `feval` and `deal` (the last two pass their
  arguments on, so the callee's own gate judges them)
- **Constructors.** `zeros`, `ones`, `rand`, `NaN`, `nan`, `Inf`, `inf`,
  `true` and `false` take three or more sizes or a size vector of any length:
  `zeros(2, 3, 4)`, `ones([2 2 2])`. Trailing sizes of 1 are dropped, as
  today, so `zeros(2, 3, 1)` is 2x3 and `zeros(2, 3, 1, 4)` is 2x3x1x4; a size
  of 0 makes an empty N-D array, `zeros(2, 3, 0)` being 2x3x0. `eye` stays
  2-D, and `cell` with a third size other than 1 keeps today's `N-D arrays
  are not supported.`
- **Shape queries**, by the MathWorks `size` page's rules: `size(A)` is a row
  of every dimension; with fewer outputs than `ndims(A)`, "all remaining
  dimension lengths are collapsed into the last argument", so `[r, c] =
  size(zeros(2, 3, 4))` gives `c` 12; with more outputs than dimensions "the
  extra trailing arguments are returned as 1"; `size(A, k)` with `k` past
  `ndims(A)` is 1. `ndims(A)` is new: 2 for every 2-D value, cells, structs,
  handles and exceptions included. `numel` is the product of every dimension;
  `length` is 0 when any dimension is 0 and the largest dimension otherwise;
  `isempty` is true when any dimension is 0; `isscalar` and `isvector` are
  false for every N-D array
- **`reshape`** to and from N-D: several sizes, a size vector, and one `[]`
  placeholder anywhere, with today's messages for a count that does not
  match and for a second placeholder; trailing sizes of 1 dropped
- **Indexing with any number of subscripts**, reading, assigning, growing
  and deleting, through the one index pipeline. Invariant 2 holds: one-based
  subscripts become zero-based only in `eval_index_args` and its helpers.
  - With `k` subscripts, `2 <= k < ndims(A)`, the dimensions from the `k`-th
    on are folded into the last subscript, so `A(i, j)` of a 2x3x4 indexes it
    as 2x12; with `k > ndims(A)`, every subscript past the dimensions must
    select position 1 (cycle 03's trailing-singleton rule, of which this is
    the general case)
  - `end` in position `p` of `k` subscripts is the size of dimension `p`,
    and in the last position the product of every dimension from `p` on; a
    single subscript's `end` is `numel(A)`
  - A single subscript is linear over every element in column-major order;
    `A(:)` is a column of every element; a logical mask selects the
    positions `find` would give. The result's shape follows cycle 03's rules
    with `numel` in place of `rows * cols`
  - A read with `k >= 2` subscripts has one dimension per subscript, each
    the number of positions it selects, trailing dimensions of 1 dropped:
    `A(:, :, 2)` of a 2x3x4 is 2x3, `A(1, :, :)` is 1x3x4, `A(:, 1, :)` is
    2x1x4
  - An assignment that selects past the end grows the array, with zeros (or
    the class's zero) filling what is new, a new page included: `B = zeros(2,
    2); B(:, :, 2) = 1` makes 2x2x2, `B(1, 1, 1, 3) = 5` makes 2x2x2x3. The
    grown shape is judged by `check_dims` before anything is allocated. A
    linear subscript past the end of an N-D array is today's ambiguous-growth
    error
  - Deletion with exactly one subscript that is not `:` removes those
    positions along its dimension, `A(:, :, 2) = []`, `A(1, :, :) = []`; with
    one linear subscript it leaves a row, as for a matrix today; with two or
    more non-colon subscripts it is today's `A null assignment can have only
    one non-colon index.`
- **Element-wise operators across every dimension**, real and complex:
  `+`, `-`, `.*`, `./`, `.\`, `.^`, the comparisons, `&`, `|`, `~` and unary
  `-` and `+`, with broadcasting: two dimensions agree when they are equal or
  one of them is 1, and a dimension past an operand's `ndims` is 1. The
  operand-size message names every dimension: `Arrays have incompatible sizes
  for operator '+' (2x3x4 vs 2x3x5).`
- **What an N-D operand is refused by, in this cycle:** the matrix
  operators, where they are not element-wise, refuse with `Matrix operations
  are not defined for N-D arrays.`: `*` unless one operand is a scalar, `/`
  unless the divisor is a scalar, `\` unless the left operand is a scalar
  (so `2 * A`, `A * 2`, `A / 2` and `2 \ A` are element-wise, as they are for
  a matrix today), and `^` whenever an operand is N-D; `'` and `.'`, with
  `Transpose is not defined for N-D arrays.`; a bracket concatenation holding
  one, with `Concatenation of N-D arrays is not supported.`; and `save` of a
  workspace holding one, with the gate's `N-D arrays are not supported by
  'save'.`, since a MAT-file of it would otherwise be written wrong (`load`
  keeps its refusal of a file holding an N-D array). The colon operator
  already takes scalars only, and `if` and `while` already judge every
  element
- **`for` over an N-D array** iterates the columns of its 2-D fold, as the
  MathWorks `for` page says: "The loop executes a maximum of n times, where n
  is the number of columns of valArray, given by numel(valArray(1,:))". Each
  value is a column of `rows` elements
- **The display**, a stated rule taken from the MathWorks page
  "Multidimensional Arrays" as fetched at planning (2026-09-30), whose page
  layout is: the page header, a blank line, the rows, two blank lines, the
  next page's header. A named display of an N-D array writes, for each page
  in column-major page order, `name(:,:,k) =` with every index past the
  second written (`name(:,:,1,2) =`), a blank line, and the page's body
  exactly as the named display of that page alone, a 2-D value of the
  array's class and storage, writes it after its `name =` line (so its own
  scale factor, column wrapping, class line and quoted char rows), and one
  more blank line between pages. The documentation's leading `A =` line
  above the first page is its live-editor layout, which also writes `szC =
  1×3` on one line where the command window writes `szC =`, a blank line and
  the row, so it is not written. A page of a complex array is shown as
  complex, whatever its own imaginary parts. `disp` of an N-D array writes
  the same pages with the headers `(:,:,k) =`. An empty N-D array's display
  is `  2×0×3 empty double array` (and `logical`, `char`), where a 2-D empty
  keeps `matrix`
- **Where sizes are shown elsewhere:** `whos` writes `2x3x4`; the protocol's
  `workspace` answers `"size":[2,3,4]`, the shape it already had; the page's
  workspace pane shows `2×3×4`; the preview's rule 4 writes `2×3×4 double`,
  and rule 1 never applies to an N-D array; a cell's display writes an
  element as `{2×3×4 double}` and a struct's a field as `[2×3×4 double]`
- **`isequal`** compares every dimension before any element, so arrays of
  different shapes are unequal, `isequal(zeros(2,2,2), zeros(2,4))` is
  false, and no comparison reads past either array
- **The rows and cases this retires.** The Known bugs row "Constructors take
  two sizes only" narrows to `repmat` and `cell`, left to 14b and to a later
  cycle; the Known deviations row "Indexing into or growing a second page"
  is removed. The cycle 01c cases `err_nd_third_size`,
  `err_nd_zero_third_size`, `err_nd_fourth_size` and `err_nd_reshape` pinned
  the refusal this cycle replaces with the array itself, and are removed,
  each named in the commit with that reason; `err_nd_size_vector`
  (`repmat`) stays until 14b. The unit tests that assert the old refusal are
  updated to the new behaviour or to the gate's message

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.
- Every builtin not on this cycle's `ND_OK`, which the gate refuses: the
  reductions (`sum`, `prod`, `mean`, `max`, `min`, `any`, `all`, `cumsum` and
  the rest), the element-wise math (`abs`, `sqrt`, `exp`, `floor`, `mod` and
  the rest), `squeeze`, `permute`, `cat`, `repmat`, `find`, `sort`, and every
  linear-algebra, numerics, string, set and plotting builtin: cycle 14b or
  later.
- Bracket concatenation of N-D arrays, and N-D MAT-files: 14b.
- N-D cell and struct arrays.
- The matrix operators and transposes on N-D arrays, which MATLAB refuses
  too.

## Design notes

Recorded as decisions are made. Start from the
"Key designs to preserve" section of docs/ARCHITECTURE.md and record here:
which files and types changed, which invariants were preserved (column-major
storage, 1-based to 0-based conversion at the index boundary, the `end` stack,
name resolution order, the output sink), and any deviation from this spec
that was accepted deliberately.

Settled at planning:

- **Why two cycles.** A survey of the crate found about 620 reads of `rows`
  and `cols` outside tests, the index pipeline's `numel` and `end` computed
  from them, and every element-wise primitive (`map`, `zip`, `transpose`,
  `to_class`) rebuilding its result from them. The operators, indexing,
  display and the workspace readers are not builtins, so no gate covers
  them: they must be taught N-D, or refuse it, in the first cycle. The
  builtins can wait behind the gate, and do.
- **Why the gate.** Cycle 10's lesson: a kernel that reads only part of a
  value's storage loses the rest in silence. One list, one check before every
  builtin, and a refusal that names the builtin make the rest of the
  library safe by default, and 14b grows the list.
- **Trailing dimensions beside `rows` and `cols`,** not a `dims` vector in
  their place, so every 2-D kernel keeps compiling and stays correct for 2-D
  values; the normalisation means a 2-D value can never carry a stored 1.
- **The places outside the gate that must decide,** from the survey: the
  index pipeline (`eval_index_args`, `resolve_read`, `resolve_write`,
  `resolve_delete`, `gather`, `scatter`, the plans), the element-wise
  primitives, the operators in `Interp::binary`, `hcat` and `vcat`, the `for`
  loop, the display, `whos`, `save`, the protocol's `workspace`, the preview,
  the cell and struct summaries, `values_equal` (which today indexes the
  second array by the first's count, and so would read past a smaller one),
  `Value::numel` and `Matrix::is_vector`.
- **The display's unverified edges** go in the Known deviations table, verify
  first: the per-page class line of a logical or char page, the `disp`
  headers, and the empty N-D wording are this spec's stated rules; the page
  layout itself is the documentation's.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/14-nd-arrays/`, as a `.m` case unless it says otherwise.
Expected output follows from this spec's rules and the existing display,
which each page reuses.

1. Constructors: `size` of `zeros(2, 3, 4)`, `ones([2 2 2])`, `NaN(2, 1, 3)`, `true(1, 1, 3)` (class `logical`), `rand(2, 2, 2)` (every element in (0, 1), self-checked), `zeros(2, 3, 1, 4)` (`2 3 1 4`), `zeros(2, 3, 1)` (`2 3`) and `ones(2, 3, 0)` (`2 3 0`), each printed with `disp`.
2. `err_*` cases: `zeros(1e5, 1e5, 1e5)` → the size message with every size; `cell(2, 3, 4)` → `N-D arrays are not supported.`
3. Shape queries: `[r, c] = size(zeros(2, 3, 4))` → `c` 12; `[a, b, c, d] = size(zeros(2, 3, 4))` → `d` 1; `size(zeros(2, 3, 4), 3)` → 4 and `size(zeros(2, 3, 4), 5)` → 1; `ndims` of `zeros(2, 3, 4)`, `5`, `{}` and `'ab'` → 3, 2, 2, 2; `numel(zeros(2, 3, 4))` → 24; `length(zeros(2, 5, 3))` → 5 and `length(zeros(2, 0, 3))` → 0; `isempty(zeros(2, 0, 3))` → 1; `isvector(zeros(1, 1, 3))` and `isscalar(zeros(1, 1, 3))` → 0.
4. `reshape`: `A = reshape(1:24, 2, 3, 4)` then `size`; `reshape(A, 6, [])` → 6x4; `reshape(A, [4 6])` → 4x6; `reshape(1:6, 1, 2, 3)` → 1x2x3; `reshape(1:6, 4, [])` → today's count message.
5. Indexing reads on `A = reshape(1:24, 2, 3, 4)`: `A(2, 3, 4)` → 24; `A(:, :, 2)` → the 2x3 page `[7 9 11; 8 10 12]`; `size(A(1, :, :))` → `1 3 4`; `v = A(1, 2, :); disp(v(:)')` → `3 9 15 21`; `A(2, 7)` → 14 (folded); `A(end)` → 24; `A(1, end)` → 23; `A(end, end, end)` → 24; `A(A > 20)'` → `21 22 23 24`; `A(1, 1, 1, 1)` → 1; `A(1, 1, 5)` → today's index-exceeds message naming position 3.
6. Indexed growth: `B = zeros(2, 2); B(:, :, 2) = [1 2; 3 4];` → `size` `2 2 2` and `B(:, :, 2)` back; `B(1, 1, 3) = 9;` → `2 2 3` with `B(2, 2, 3)` 0; `B(1, 1, 1, 2) = 5;` → `2 2 3 2`; a char and a logical grown into a second page keep their class.
7. Deletion: on `A = reshape(1:24, 2, 3, 4)`, `A(:, :, 2) = []` → `2 3 3` and `A(:, :, 2)` now the old third page; `A(1, :, :) = []` → `1 3 3`; `A(3) = []` on a fresh 2x2x2 → a 1x7 row; `A(1, 2, :) = []` → `A null assignment can have only one non-colon index.`
8. Element-wise operators: `A + 1`, `A .* A`, `-A`, `A > 12` (class logical, `size` `2 3 4`), `~(A > 12)`, `2 * A` and `A / 2` on `A = reshape(1:8, 2, 2, 2)`, each checked through its `A(:)'` row; broadcasting `A - [10; 20]` (2x1 against 2x2x2) and `zeros(2, 3, 4) + ones(1, 1, 4)` (`size` `2 3 4`); a complex N-D array, `A * 1i`, keeping its shape and its imaginary parts.
9. `err_*`: `zeros(2, 3, 4) + zeros(2, 3, 5)` → `Arrays have incompatible sizes for operator '+' (2x3x4 vs 2x3x5).`
10. The display: `A = reshape(1:8, 2, 2, 2)` named, with both pages; `disp(A)`; `L = reshape(logical([1 0 1 0 1 0 1 0]), 2, 2, 2)` named; `c = 'ab'; c(:, :, 2) = 'cd'` named; `E = zeros(2, 0, 3)` named; `Z = zeros(1, 1, 2); Z(:, :, 2) = 1i` named, both pages complex; `x = zeros(1, 1, 1, 2)` named, with the headers `x(:,:,1,1) =` and `x(:,:,1,2) =`.
11. Elsewhere: `c = {zeros(2, 3, 4)}` and `s.f = zeros(2, 3, 4)` displayed; `whos` after `A = zeros(2, 3, 4);`; a `.proto` case: `workspace` with `"preview":true` after `A = zeros(2, 3, 4);` → `"size":[2,3,4]` and `"value":"2×3×4 double"`.
12. The gate and the refusals, each an `err_*` case: `sum(zeros(2, 2, 2))` → `N-D arrays are not supported by 'sum'.`; `feval(@abs, zeros(2, 2, 2))` → the same for `'abs'`; `zeros(2, 2, 2) * ones(2, 2)` and `zeros(2, 2, 2) ^ 2` → `Matrix operations are not defined for N-D arrays.`; `zeros(2, 2, 2)'` → `Transpose is not defined for N-D arrays.`; `[zeros(2, 2, 2), 1]` → `Concatenation of N-D arrays is not supported.`; `A = zeros(2, 2, 2); save('scratch_nd.mat', 'A')` → `N-D arrays are not supported by 'save'.`, with no file left behind.
13. `for`: `for col = reshape(1:8, 2, 2, 2), disp(col'), end` → four lines `1 2`, `3 4`, `5 6`, `7 8`.
14. `isequal`: `isequal(zeros(2, 2, 2), zeros(2, 2, 2))` → 1; `isequal(zeros(2, 2, 2), zeros(2, 4))` → 0; `isequal(zeros(2, 2, 2), zeros(2, 2))` → 0; `isequal(zeros(2, 2), zeros(2, 2, 2))` → 0.
15. The pass-through builtins: `double(true(1, 1, 2))`, `logical(zeros(1, 1, 2))`, `char(zeros(1, 1, 2) + 65)` keep their shape; `fprintf('%d ', reshape(1:8, 2, 2, 2)); fprintf('\n')` → `1 2 3 4 5 6 7 8`; `class(zeros(2, 2, 2))` → `double`.
16. Unit tests: the normalisation (no stored trailing 1, a 2-D value unchanged); `check_dims` at and past `MAX_ELEMS`; the column-major offset of an N-D subscript; the fold of trailing dimensions and `end`; broadcasting's result shape; the gate refusing an N-D argument to a builtin not listed and passing one listed; `values_equal` on arrays of different shapes.
17. Every existing case passes unchanged, apart from the four 01c cases this spec removes.

## Status

Planned
