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

Settled in testing, before the code that follows it:

- **A colon over an empty target takes the right-hand side's extent in
  every position where the target has none of its own,** a dimension of 0
  or one past the target's dimensions, the general case of cycle 03's rule
  for a matrix, so `x = []; x(:, :, :) = reshape(1:8, 2, 2, 2)` builds the
  2x2x2 array; the first build took the extent only where the fold's
  dimension was 0 and refused it. A dimension the empty target has keeps
  its size, as in cycle 03, so `x = zeros(0, 3); x(:, :) = 5` is still
  `[5 5 5]`.
- **The display is written a page at a time,** so its memory is one page's
  whatever the page count, and its time is linear in what it writes; an
  array with very many dimensions writes a long header per page, which the
  layout above requires.
- **Nothing a program controls reaches a formatting width,** since Rust's
  formatter refuses any width past 65,535: `whos` pads its size column by
  hand, as the display already pads its columns.
- **A cell holding an N-D char is compared by every dimension** in `strcmp`
  and `strcmpi`, as the set functions and `str2double` judge one.

Recorded during implementation:

- **Files and types changed.** `src/value.rs`: `Matrix` gains the private
  `higher: Vec<usize>`, the dimensions past the second, which only
  `from_dims`, `filled_dims` and `set_dims` set, each normalising through
  `normalize_dims`; `dims`, `ndims`, `is_nd`, `is_char_row`, `fold_cols`,
  `pages`, `page`, `page_display` and `write_pages` are new, as are
  `Value::write_display` and `Value::write_disp` and the free functions
  `dims_product`, `normalize_dims`, `dims_text`, `broadcast_dims`,
  `broadcast_walk`, `push_left` and `push_right`.
  `map`, `try_map`, `map_c`, `real_part`, `imag_part` and `to_class` carry
  every dimension, and `try_zip` (so `zip`) and `zip_c` broadcast across
  every dimension through `broadcast_walk`, a counter carried one step per
  element, the 2-D loop kept for two 2-D operands. `Value::dims` returns a
  `Vec<usize>` of every dimension, so the compiler listed each caller;
  `Value::numel` is the product and `Value::is_blank` is false for an N-D
  empty. `src/builtins/args.rs`: `check_dims` and `shape_dims`;
  `check_shape` is `check_dims` of two sizes. `src/builtins/mod.rs`:
  `ND_OK`, `nd_gate`. `src/builtins/core.rs`: the N-D constructors, `size`,
  `ndims` (new), `numel`, `length`, the shape predicates, `values_equal`
  and `whos`. `src/builtins/linalg.rs`: `reshape`. `src/builtins/io.rs`
  and `mat.rs`: `save` refusing an N-D array. `src/builtins/sets.rs`,
  `strings.rs` (`str2double`, `strcmp`, `strcmpi`) and `cells.rs`
  (`cell2mat`, `num2cell`, `map_elements`): their judgments of an N-D
  element, below. `src/interp.rs`: `Sel::List`
  holds its index's `shape`, the plans `Gather`, `Scatter` and `Keep` hold
  `dims`, and `end_value`, `fold_dims`, `sel_positions`, `sel_counts`,
  `keeps_layout`, `flat`, `flat_operand`, `emit_display`, `emit_disp` and
  `emit_to` are new; `resolve_read`, `resolve_write`, `resolve_delete`,
  `regrid`, `scatter`, `gather`, the operators, `for`, `switch`, `hcat`,
  `vcat` and `show_var` changed. `src/error.rs`:
  `nd_argument`, `nd_matrix_operation`, `nd_transpose` and
  `nd_concatenation` are new, and `size_overflow`, `operator_dims` and
  `reshape_numel` name every dimension. `src/env.rs`, `src/protocol.rs`
  and `src/ui/app.js`: the preview, the workspace's `size` and the pane's
  size text.
- **Invariants preserved.** Column-major storage: every resolver and
  `regrid` compute offsets as `i1 + d1*(i2 + ...)`, and the fold of
  trailing dimensions costs nothing because a position's offset is the
  same in the array and in its fold. One-based to zero-based: still only
  in `eval_index_args` through `index_positions` and `mask_positions`;
  `end_value` computes one-based sizes, never positions. The `end` stack:
  pushed per subscript in `eval_index_args` only, now with `end_value`. Name
  resolution and the output sink: unchanged; the display still goes
  through `Value::display` and `Interp::emit`. Invariant 6: every shape
  computed here, a constructor's, a reshape's, a broadcast's, a read's and
  a growth's and its count of positions, goes through `check_dims` before
  anything is allocated; no `unwrap` or `expect` was added on anything a
  program controls, and a product of the dimensions of an empty array,
  which can pass `usize`, saturates rather than overflowing.
- **Growth through fewer subscripts than dimensions.** The spec gives the
  linear case, the ambiguous-growth error. With two or more subscripts but
  fewer than the array's dimensions, the last indexes a fold that is not a
  dimension of the array, so a subscript past the end of any of them is the
  same ambiguous-growth error (`A(3, 1) = 1` and `A(1, 13) = 1` of a
  2x3x4). A linear subscript past the end of an N-D empty is that error too:
  the N-D test comes before the empty one. Not verified against MATLAB.
- **Deletion through fewer subscripts** works on the fold, and leaves the
  folded shape: `A(:, 2) = []` of a 2x3x4 is 2x11. When every subscript
  selects its whole dimension, the last one written as a list deletes, and
  the first dimension when all are `:`, which is cycle 03's two-subscript
  rule generalised; a subscript past `ndims` must select position 1 and
  never deletes, as before. Not verified against MATLAB.
- **Reads.** An N-D index gives its own shape, as cycle 03's rule has it
  (`A(ones(2, 2, 2))` is 2x2x2), and an N-D mask selects as a column, as
  `find` of a mask that is not a row gives. `A(:, :, [1 1])` of a matrix is
  now the 2-D matrix doubled into two pages, the general case of the
  trailing-singleton rule.
- **The count of positions an assignment selects** is judged by
  `check_dims` as a shape, since repeated subscripts can make it far larger
  than the array; before, `A(ones(1, 1e5), ones(1, 1e5)) = 5` asked the
  allocator for 1e10 positions.
- **`[A]` of one N-D array** is `A`, stored by the flag rule as a
  bracket's result is: a bracket of one joins it to nothing, so it is no
  concatenation, and `cell2mat({A})` is `A` by the same path. With anything
  beside it, `[]` included, it is the concatenation refusal.
- **`disp` of an N-D array** writes exactly the pages the named display
  writes, the trailing blank line of the last page included, under
  `(:,:,k) =`; `disp` of an empty N-D array writes nothing, as `disp` of any
  other empty but a char does, whatever its class.
- **Cells and structs.** The index pipeline takes their two dimensions, and
  `flat` refuses, with today's `N-D arrays are not supported.`, a read or a
  growth whose shape would be N-D (`c(:, :, [1 1])`, `c{1, 1, 2} = 5`), so
  their paths refuse exactly what they refused before. The gate judges only
  a matrix argument, as the complex gate does; a builtin that reads a cell
  or struct holding an N-D array judges it itself: `cell2mat` through the
  bracket rule, `save` in its MAT-file writer, which refuses an N-D array
  wherever it sits, a variable, a cell element or a field, before a byte is
  written, and in `-ascii`; `-append` leaves the old file as it was. The
  set functions (`sets.rs`) refuse a cell holding an N-D char and
  `str2double` (`strings.rs`) reads one as `NaN`, each through
  `Matrix::is_char_row`, since an N-D char is no character vector however
  few its rows; `strcmp` and `strcmpi` compare one by every dimension; and
  `cellfun`, `arrayfun` and `num2cell` (`cells.rs`) take the two
  dimensions of a cell, which is never N-D, or of an array the gate has
  let through, and `cellfun` hands each element on to a callee whose own
  gate judges it.
- **The gates' order.** `complex_gate` runs first, so a complex N-D
  argument to a builtin on neither list is the complex refusal. The
  operators judge an N-D operand before the complex dispatch, so a complex
  and a real one are refused alike; `^` is refused whenever an operand is
  N-D, a scalar base or exponent included, as the spec says.
- **Smaller choices.** A size vector with one row that is N-D (`1x1x3`) is
  not a row vector, the existing refusal, since `size_list` read it as one.
  `size` computes its outputs in `f64`, so the fold of an empty's huge
  dimensions is named as it is. Trailing ones are dropped before
  `check_dims`, so a refusal names the sizes the array would have. `for`
  over an N-D array with no rows follows the 2-D rule on the fold. A char
  that is N-D is no character vector for `switch`, a cell display, a
  struct field or the preview. `eye` and `cell` keep `args::shape`, so
  `cell(2, 3, 1)` is still the N-D refusal, as it was.
- **The builtin count in `docs/FEATURES.md`** read 231, which left out
  cycle 13's nineteen; it is 251 with `ndims`.
- **`whos` pads by hand**, as settled in testing. `whos_text` padded its
  size column with a formatted width, and Rust's formatter panics on a
  runtime width past 65,535: `A = zeros([2 ones(1, 40000) 2]); whos`
  exited 101, and under `--protocol` or `--ui` took the process down. Every
  column is padded through `value::push_left` and `push_right` now, and
  so are a struct's field lines, whose width is a field name's, which has
  no length limit either: `s.(repmat('a', 1, 70000)) = 1` panicked the
  same way. A search of the crate for every other runtime width or
  precision found none a program controls: `printf`'s are bounded by
  `printf::MAX_FIELD`, `num2str`'s and `mat2str`'s precision by its clamp
  at 800, `save -ascii`'s width is fixed, and the display's widths and
  decimals follow from the numbers' own texts.
- **`strcmp` and `strcmpi` compare every dimension**, as settled in
  testing. `same_text` judged two chars one size by their rows and
  columns, so `strcmp({reshape('abcd', 1, 1, 4)}, {reshape('abcd', 1, 1,
  2, 2)})` was 1; it compares `dims` now, and two empties are still the
  same text whatever their shapes. The other readers of a cell's elements
  were searched for the same judgment and none makes it: `strncmp` and
  `strncmpi` compare the first `n` units whatever the shape, as they do
  for a char matrix, and `upper`, `lower`, `strtrim`, `strrep`, `strcat`,
  `strjoin`, `strfind`, `regexp`, `regexprep`, `isfield`, `rmfield` and
  `legend` read an N-D char's units in column-major order, as they read a
  char matrix's, with no test of its rows or columns.
- **A colon over an empty target**, as settled in testing, takes the
  right-hand side's extent in every position where the target has no
  extent of its own: a dimension of 0, as in cycle 03, and every position
  past its dimensions, which `resolve_write` had given the extent 1, so
  that `x = []; x(:, :, :) = reshape(1:8, 2, 2, 2)` was refused with `left
  side has 4 elements and the right side has 8`. A dimension an empty
  target has keeps its extent, as cycle 03 has it, so `x = zeros(0, 3);
  x(:, :) = 5` still fills three columns, and past the dimensions of an
  array with elements a colon still selects its one position. Not
  verified against MATLAB.
- **The display is written a page at a time**, as settled in testing.
  `page_display` built the whole display in one text, each header joined
  from a list of index texts, so `A = zeros([1 1 ones(1, 10000) 10000])`
  built 200 MB before writing a byte of it: 46 s and a peak of 320 MB in
  a debug build, and `disp(A)` 57 s. `Matrix::write_pages` hands each
  page, its header and its body, to `Interp::emit_display` or `emit_disp`,
  which the statement displays and `disp` call, and `show_var` writes a
  variable where it is stored through `emit_to`, the body of
  `Interp::emit`, rather than copy an array as large. A header writes a
  run of dimensions of 1 as one piece and steps only the dimensions past
  1, so its cost is linear in its length. The same display takes 4 s with
  a peak of 7 MB, and `disp(A)` 3 s, each byte for byte as before;
  `page_display` gathers the pages for a caller that wants the text.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/14-nd-arrays/`, as a `.m` case unless it says otherwise.
Expected output follows from this spec's rules and the existing display,
which each page reuses.

1. Constructors: `size` of `zeros(2, 3, 4)`, `ones([2 2 2])`, `NaN(2, 1, 3)`, `true(1, 1, 3)` (class `logical`), `rand(2, 2, 2)` (every element in (0, 1), self-checked), `zeros(2, 3, 1, 4)` (`2 3 1 4`), `zeros(2, 3, 1)` (`2 3`) and `ones(2, 3, 0)` (`2 3 0`), each printed with `disp`. Cases: `constructors_nd_sizes.m`, `constructors_nd_empty_huge.m`.
2. `err_*` cases: `zeros(1e5, 1e5, 1e5)` → the size message with every size; `cell(2, 3, 4)` → `N-D arrays are not supported.` Cases: `err_constructor_size_every_dim.m`, `err_cell_third_size.m`.
3. Shape queries: `[r, c] = size(zeros(2, 3, 4))` → `c` 12; `[a, b, c, d] = size(zeros(2, 3, 4))` → `d` 1; `size(zeros(2, 3, 4), 3)` → 4 and `size(zeros(2, 3, 4), 5)` → 1; `ndims` of `zeros(2, 3, 4)`, `5`, `{}` and `'ab'` → 3, 2, 2, 2; `numel(zeros(2, 3, 4))` → 24; `length(zeros(2, 5, 3))` → 5 and `length(zeros(2, 0, 3))` → 0; `isempty(zeros(2, 0, 3))` → 1; `isvector(zeros(1, 1, 3))` and `isscalar(zeros(1, 1, 3))` → 0. Cases: `shape_queries_nd.m`.
4. `reshape`: `A = reshape(1:24, 2, 3, 4)` then `size`; `reshape(A, 6, [])` → 6x4; `reshape(A, [4 6])` → 4x6; `reshape(1:6, 1, 2, 3)` → 1x2x3; `reshape(1:6, 4, [])` → today's count message. Cases: `reshape_nd.m`, `reshape_nd_empty_huge.m`, `err_reshape_nd_count.m`, `err_reshape_nd_placeholder_count.m`, `err_reshape_two_placeholders.m`, `err_reshape_nd_numel.m`.
5. Indexing reads on `A = reshape(1:24, 2, 3, 4)`: `A(2, 3, 4)` → 24; `A(:, :, 2)` → the 2x3 page `[7 9 11; 8 10 12]`; `size(A(1, :, :))` → `1 3 4`; `v = A(1, 2, :); disp(v(:)')` → `3 9 15 21`; `A(2, 7)` → 14 (folded); `A(end)` → 24; `A(1, end)` → 23; `A(end, end, end)` → 24; `A(A > 20)'` → `21 22 23 24`; `A(1, 1, 1, 1)` → 1; `A(1, 1, 5)` → today's index-exceeds message naming position 3. Cases: `index_read_nd.m`, `err_index_page_past_end.m`, `err_index_folded_past_end.m`, `err_index_linear_past_end.m`, `err_index_trailing_past_one.m`.
6. Indexed growth: `B = zeros(2, 2); B(:, :, 2) = [1 2; 3 4];` → `size` `2 2 2` and `B(:, :, 2)` back; `B(1, 1, 3) = 9;` → `2 2 3` with `B(2, 2, 3)` 0; `B(1, 1, 1, 2) = 5;` → `2 2 3 2`; a char and a logical grown into a second page keep their class. Cases: `grow_nd_pages.m`, `grow_nd_keeps_class.m`, `err_grow_nd_linear_ambiguous.m`, `err_grow_nd_size_every_dim.m`, `grow_nd_colon_empty.m`.
7. Deletion: on `A = reshape(1:24, 2, 3, 4)`, `A(:, :, 2) = []` → `2 3 3` and `A(:, :, 2)` now the old third page; `A(1, :, :) = []` → `1 3 3`; `A(3) = []` on a fresh 2x2x2 → a 1x7 row; `A(1, 2, :) = []` → `A null assignment can have only one non-colon index.` Cases: `delete_nd.m`, `err_delete_nd_two_indices.m`.
8. Element-wise operators: `A + 1`, `A .* A`, `-A`, `A > 12` (class logical, `size` `2 3 4`), `~(A > 12)`, `2 * A` and `A / 2` on `A = reshape(1:8, 2, 2, 2)`, each checked through its `A(:)'` row; broadcasting `A - [10; 20]` (2x1 against 2x2x2) and `zeros(2, 3, 4) + ones(1, 1, 4)` (`size` `2 3 4`); a complex N-D array, `A * 1i`, keeping its shape and its imaginary parts. Cases: `elementwise_nd_arithmetic.m`, `elementwise_nd_logical.m`, `broadcast_nd.m`, `complex_nd_elementwise.m`.
9. `err_*`: `zeros(2, 3, 4) + zeros(2, 3, 5)` → `Arrays have incompatible sizes for operator '+' (2x3x4 vs 2x3x5).` Cases: `err_operator_nd_sizes.m`, `err_operator_nd_sizes_2d_operand.m`.
10. The display: `A = reshape(1:8, 2, 2, 2)` named, with both pages; `disp(A)`; `L = reshape(logical([1 0 1 0 1 0 1 0]), 2, 2, 2)` named; `c = 'ab'; c(:, :, 2) = 'cd'` named; `E = zeros(2, 0, 3)` named; `Z = zeros(1, 1, 2); Z(:, :, 2) = 1i` named, both pages complex; `x = zeros(1, 1, 1, 2)` named, with the headers `x(:,:,1,1) =` and `x(:,:,1,2) =`. Cases: `display_nd_double.m`, `disp_nd_pages.m`, `display_nd_logical.m`, `display_nd_char.m`, `display_nd_empty.m`, `display_nd_complex.m`, `display_nd_fourth_dim.m`, `display_nd_page_scale.m`, `display_nd_page_columns.m`.
11. Elsewhere: `c = {zeros(2, 3, 4)}` and `s.f = zeros(2, 3, 4)` displayed; `whos` after `A = zeros(2, 3, 4);`; a `.proto` case: `workspace` with `"preview":true` after `A = zeros(2, 3, 4);` → `"size":[2,3,4]` and `"value":"2×3×4 double"`. Cases: `cell_struct_nd_summary.m`, `whos_nd_size.m`, `workspace_nd_size.proto`, `whos_nd_long_size.m`, `struct_field_name_past_width.m`.
12. The gate and the refusals, each an `err_*` case: `sum(zeros(2, 2, 2))` → `N-D arrays are not supported by 'sum'.`; `feval(@abs, zeros(2, 2, 2))` → the same for `'abs'`; `zeros(2, 2, 2) * ones(2, 2)` and `zeros(2, 2, 2) ^ 2` → `Matrix operations are not defined for N-D arrays.`; `zeros(2, 2, 2)'` → `Transpose is not defined for N-D arrays.`; `[zeros(2, 2, 2), 1]` → `Concatenation of N-D arrays is not supported.`; `A = zeros(2, 2, 2); save('scratch_nd.mat', 'A')` → `N-D arrays are not supported by 'save'.`, with no file left behind. Cases: `err_gate_sum.m`, `err_gate_feval_abs.m`, `err_gate_second_argument.m`, `err_mtimes_nd.m`, `err_mpower_nd.m`, `err_mpower_nd_exponent.m`, `err_mrdivide_nd.m`, `err_mldivide_nd.m`, `err_ctranspose_nd.m`, `err_transpose_nd.m`, `err_hcat_nd.m`, `err_vcat_nd.m`, `err_save_nd.m`, `err_save_nd_workspace.m`, `err_set_cell_nd_char.m`, `str2double_cell_nd_char.m`, `strcmp_cell_nd_char.m`.
13. `for`: `for col = reshape(1:8, 2, 2, 2), disp(col'), end` → four lines `1 2`, `3 4`, `5 6`, `7 8`. Cases: `for_nd_columns.m`.
14. `isequal`: `isequal(zeros(2, 2, 2), zeros(2, 2, 2))` → 1; `isequal(zeros(2, 2, 2), zeros(2, 4))` → 0; `isequal(zeros(2, 2, 2), zeros(2, 2))` → 0; `isequal(zeros(2, 2), zeros(2, 2, 2))` → 0. Cases: `isequal_nd_shapes.m`.
15. The pass-through builtins: `double(true(1, 1, 2))`, `logical(zeros(1, 1, 2))`, `char(zeros(1, 1, 2) + 65)` keep their shape; `fprintf('%d ', reshape(1:8, 2, 2, 2)); fprintf('\n')` → `1 2 3 4 5 6 7 8`; `class(zeros(2, 2, 2))` → `double`. Cases: `passthrough_nd.m`.
16. Unit tests: the normalisation (no stored trailing 1, a 2-D value unchanged); `check_dims` at and past `MAX_ELEMS`; the column-major offset of an N-D subscript; the fold of trailing dimensions and `end`; broadcasting's result shape; the gate refusing an N-D argument to a builtin not listed and passing one listed; `values_equal` on arrays of different shapes. Tests: `nd_dimensions_are_stored_without_a_trailing_one`, `check_dims_judges_every_size_before_allocating`, `an_nd_subscript_lands_at_its_column_major_offset`, `fewer_subscripts_fold_the_trailing_dimensions_and_end`, `broadcasting_runs_across_every_dimension`, `broadcasting_passes_over_singleton_dimensions`, `singleton_subscripts_and_dimensions_cost_nothing_extra`, `the_gate_refuses_an_nd_argument_to_every_other_builtin`, `values_equal_compares_every_dimension_first`.
17. Every existing case passes unchanged, apart from the four 01c cases this spec removes. Cases: the whole golden suite.

## Status

Done (2026-09-30)
