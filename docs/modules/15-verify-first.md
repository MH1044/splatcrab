# 15 — Verify first

## Goal

Settle what the MathWorks documentation settles. Many rows of the Known
bugs and Known deviations tables in `docs/ARCHITECTURE.md` are marked
verify first: each records a behaviour chosen without a source for
MATLAB's, and says not to assert either answer until one is found. This
cycle takes those rows to their documentation pages, quoted below, and
applies one rule to each: a behaviour a quoted sentence settles is made to
match it, with a golden case; a behaviour a page confirms as today's is
pinned by a golden case and its row removed; a row no page settles stays,
with the page cited as not settling it; and a behaviour a page settles but
that needs real work is recorded for a later cycle rather than built here.

After this module a user can do things that were not possible before; every
bullet in Scope must be demonstrable by at least one acceptance test below.

## Sources

Quoted at planning, on 2026-09-30, from the MathWorks reference pages.
Each is cited by its label in Scope and in the tables.

- **S1, the `colon` page.** Version History, R2025a: "`colon` now returns
  an error when creating vectors if one or more operands are not scalar.
  For example, expressions like `[1 2 3]:2:10` now error. Previously,
  `colon` used the first element of any nonscalar operands to evaluate the
  expression." Input Arguments: `j`, `i` and `k` are each "specified as a
  real numeric scalar". Its empty results: "If `j > k`, then `x = j:k` is
  an empty matrix", and `j:i:k` "returns an empty matrix when: `i == 0`",
  "`i > 0` and `j > k`", "`i < 0` and `j < k`". The page says nothing of a
  `NaN` or `Inf` operand.
- **S2, the `for` page.** "`valArray` — Create a column vector, `index`,
  from subsequent columns of array `valArray` on each iteration. For
  example, on the first iteration, `index = valArray(:,1)`. The loop
  executes a maximum of `n` times, where `n` is the number of columns of
  `valArray`, given by `numel(valArray(1,:))`."
- **S3, the `isequal` page.** "The equality of two function handles depends
  on how they are constructed." "Tables, timetables, structures, and cell
  arrays are equivalent only when all elements and properties are equal."
  "Structures — Fields need not be in the same order as long as the
  contents are equal." "Numeric inputs are equivalent if they are the same
  size and their contents are of equal value." "`isequal` does not treat
  NaN values as equal to each other." "`isequal` does not consider data
  type when it tests for equality."
- **S4, the "Compare Function Handles" page** the `isequal` page points to.
  "MATLAB® considers function handles that you construct from the same
  named function to be equal. The isequal function returns a value of
  `true` when comparing these types of handles", with `fun1 = @sin; fun2 =
  @sin; isequal(fun1,fun2)` giving `1`. "Unlike handles to named functions,
  function handles that represent the same anonymous function are not
  equal. They are considered unequal because MATLAB cannot guarantee that
  the frozen values of nonargument variables are the same", with `A = 5;
  h1 = @(x)A * x.^2; h2 = @(x)A * x.^2; isequal(h1,h2)` giving `0`. "If you
  make a copy of an anonymous function handle, the copy and the original
  are equal", with `h2 = h1` giving `1`. Its third section, on nested
  functions, has no counterpart here: SplatCrab has no nested functions.
- **S5, the `transpose` and `ctranspose` pages.** `transpose`: "`B = A.'`
  returns the nonconjugate transpose of `A`, that is, interchanges the row
  and column index for each element", and "`B = transpose(A)` is an
  alternate way to execute `A.'`". Both pages list `struct` and `cell`
  among the data types of `A`, and `ctranspose`'s Tips say: "For logical or
  non-numeric inputs, ctranspose and transpose produce the same result."
- **S6, the `sum`, `mean`, `cumsum` and `prod` pages**, on `dim`. `sum`:
  "`sum` returns `A` when `dim` is greater than `ndims(A)` or when
  `size(A,dim)` is `1`." `mean`: "`mean` returns `A` when `dim` is greater
  than `ndims(A)` or when `size(A,dim)` is `1`." `cumsum`: "`cumsum`
  returns `A` if `dim` is greater than `ndims(A)`." `prod`: "`prod` returns
  `A` when `dim` is greater than `ndims(A)`."
- **S7, the `any` and `all` pages.** `any`, on `A`: "The `any` function
  ignores elements of `A` that are `NaN` (Not a Number)." `all`: "`B =
  all(A)` tests along the first array dimension of `A` whose size does not
  equal 1, and determines if the elements are all nonzero or logical `1`
  (`true`)." Neither page states what `dim` past `ndims(A)` gives.
- **S8, the `iscellstr` page's Note.** "`iscellstr` returns a `1` (`true`)
  for a cell array containing character arrays of any size. Most
  text-processing functions and conversion functions require input cell
  arrays to contain only character row vectors and a cell array containing
  a character array with more than one row result in an error."
- **S9, the text functions' inputs.** `upper` and `lower`: "Input array,
  specified as a string array, character array, or cell array of character
  vectors." `strtrim`: "Input text, specified as a character array or as a
  cell array of character arrays, or a string array." `strrep`: `str`,
  `old` and `new` each "specified as a string array, character vector, or
  cell array of character vectors". `strfind`: `str` "specified as a string
  array, character vector, or cell array of character vectors", `pat` a
  "String scalar", a "Character vector" or a "pattern scalar". `regexp`:
  `str` and `expression` "specified as a character vector, a cell array of
  character vectors, or a string array". `regexprep`: `str`, `expression`
  and `replace` the same. `strjoin`: `C` "specified as a `1`-by-`n` cell
  array of character vectors or string array", `delimiter` "specified as a
  character vector, a `1`-by-`n` cell array of character vectors, or a
  `1`-by-`n` string array". `strcat`: "Input text, specified as character
  arrays, cell arrays of character vectors, or string arrays." `strsplit`:
  `str` "specified as a character vector or a string scalar", `delimiter`
  as `strjoin`'s. `strtok`: `str` "specified as a string array, a character
  vector, or a cell array of character vectors"; its delimiters "can be any
  size". `strcmp`: "Input text, with each input specified as a character
  vector, a character array, a cell array of character vectors, or a string
  array", and "If used on unsupported data types, strcmp always returns 0."
- **S10, the `varargin` page.** "If the function receives no inputs after
  the explicitly declared inputs, then `varargin` is an empty cell array."
  Its example `definedAndVariableNumInputs(X,Y,varargin)`, called with two
  inputs, prints `Size of varargin cell array: 0x0`, and with five, `1x3`.
- **S11, the `func2str` page's examples.** `fh = @(x,y)sqrt(x.^2+y.^2);`
  then `disp(['Anonymous function: ' c])` prints `Anonymous function:
  @(x,y)sqrt(x.^2+y.^2)`; `fh = @(x) x.^2+7;` renders as `@(x)x.^2+7`.
- **S12, the "Choose Command Syntax or Function Syntax" page.** "If you
  issue this statement at the command line, MATLAB uses syntactic rules,
  the current workspace, and path to determine whether `ls` and `d` are
  functions or variables. However, some components, such as the Code
  Analyzer and the Editor/Debugger, operate without reference to the path
  or workspace." Nothing about a name a script assigns implicitly, such as
  `ans`.
- **S13, the "Multidimensional Arrays" page.** Its displays are of double
  pages alone (`A(:,:,1) =`, a blank line, the rows); it shows no logical
  or char page, no `disp` of an N-D array, no empty N-D array, and says
  nothing of growth or deletion through fewer subscripts than dimensions.
- **S14, the `exist` page.** Its table of return values, 0 to 8, names no
  local function.
- **S15, the `mldivide` page.** "Warning: Matrix is close to singular or
  badly scaled. Results may be inaccurate. RCOND =  1.306145e-17." and
  "When `rcond` is between `0` and `eps`, MATLAB® issues a nearly singular
  warning, but proceeds with the calculation."
- **S16, the `eig` page.** Version History, R2021b: "`eig` returns `NaN`
  values when the input contains nonfinite values (`Inf` or `NaN`)." The
  page gives no shape for the vectors or the diagonal matrix of a
  two-output call with nonfinite input.

## Scope

**Settled by a page, and changed:**

- **`isequal` of function handles** (S3, S4). Two named handles, `@name`
  or `str2func('name')`, are equal when they name the same function: the
  same name, and the same local function bound where each was made, or no
  local function for either. Two anonymous functions are equal only when
  one is a copy of the other, the same handle passed on by assignment, as
  an argument, or through a cell or a field; two made separately are
  unequal whatever their text and captures. A named handle never equals an
  anonymous one, and a handle equals nothing but a handle. Where today every
  pair of handles is unequal. `==` of handles keeps today's refusal
- **`isequal` of cells and structs** (S3). Two cell arrays are equal when
  they have the same size and every pair of elements in the same position
  is equal by `isequal`'s rules: arrays by size and value with the class
  not compared and a `NaN` equal to nothing, handles by the bullet above,
  an `MException` as today, and a cell or a struct by this bullet however
  deeply nested. Two struct arrays are equal when they have the same size,
  the same field names in any order, and every field of every element is
  equal by the same rules. A cell never equals a struct or an array, and a
  struct never equals an array. Where today every such pair is unequal. The
  comparison walks nested cells and structs without recursion, so any
  nesting a program can build is compared in time proportional to its
  elements, and never overflows the stack (invariant 6)
- **Cells and structs transpose** (S5). `c'`, `c.'` and `transpose(c)` of
  a cell array give the cell array with the row and column index of every
  element interchanged, each element itself unchanged (never transposed or
  conjugated); a struct array alike, its fields and their order kept. A
  function handle and an `MException` keep today's refusal, `This operation
  is not supported for a value of class 'function_handle'.`, and an N-D
  array keeps `Transpose is not defined for N-D arrays.`
- **`sum` and `mean` along a dimension of size 1** (S6). Along a dimension
  within `ndims(A)` whose size is 1, `sum(A, dim)` and `mean(A, dim)` give
  what they give today along a dimension past `ndims(A)`: `A`'s values, a
  `-0` kept, with today's classes and storage there. With no dimension the
  default dimension is judged the same way, so `sum(-0)` and `mean(-0)` are
  `-0`, where today they are `+0`. `sum(A, 'all')`, `prod`, `max` and
  `min` are unchanged, and so are `cumsum` and `cumprod`, whose pages say
  "returns `A`" only past `ndims(A)`
- **`any` and `all` past `ndims`** (S7). With `dim` past `ndims(A)`, `any`
  and `all` give each element the answer they give it along a dimension of
  size 1 within `ndims(A)`, a logical array of `A`'s shape: a `NaN` is
  ignored by `any` and is nonzero to `all`, so `any(NaN, 3)` is false and
  `all(NaN, 3)` true, where today both are `NaN's cannot be converted to
  logicals.`; every other element's answer is unchanged. Complex arguments
  keep today's refusal
- **Text functions and char arrays of several rows** (S8, S9). A character
  vector is a char array of at most one row and two dimensions
  (`Matrix::is_char_row`): `''`, a char with no rows and a char row are
  character vectors, a column of two characters is not. Where a page below
  takes a character vector or a cell array of character vectors, a char
  array of more than one row, or of more than two dimensions, is refused as
  a value that is not a char is refused there today, with today's message
  for that place: `Argument N to '<name>' must be a character vector.` for
  an argument, `Every element of a cell argument to '<name>' must be a
  character vector.` for an element of a cell. Today each is read as its
  code units in column-major order, one row, which gave `upper({['ab';
  'cd']})` as `{'ACBD'}`. The places:
  - `upper` and `lower`: the elements of a cell (a char array argument of
    any shape keeps today's answer, its shape kept)
  - `strrep`: each of its three arguments and the elements of a cell
  - `strfind`: both arguments and the elements of a cell
  - `regexp` and `regexprep`: each text argument and the elements of a cell
  - `strjoin`: the elements of its cell and its delimiter, a character
    vector or the elements of a cell
  - `strcat`: the elements of a cell (a char array argument of several rows
    keeps today's rule)
  - `strsplit`: its text, and its delimiter, a character vector or the
    elements of a cell
  - `strtok`: its text; its delimiters, of any size, as today
  - `strtrim`, whose page takes "a cell array of character arrays": an
    element of a cell that is a char array of several rows is trimmed as
    `strtrim` trims that array on its own, its shape kept, where today it
    is flattened into one row; an element of more than two dimensions is
    refused as the gate refuses such an argument, `N-D arrays are not
    supported by 'strtrim'.`

  An N-D char argument keeps the gate's refusal. `strcmp`, `strcmpi`,
  `strncmp` and `strncmpi` (whose page takes char arrays of several rows),
  `str2double`, `isspace`, `isletter` and the number conversions are
  unchanged

**Settled by a page as today's behaviour, and pinned:**

- **A colon operand that is not a scalar is an error** (S1), today's
  `range start must be a scalar.`, `range step must be a scalar.` and
  `range end must be a scalar.`, an empty operand included, and in a `for`
  range alike; `1:size(ones(3, 4))`, the page's own example, is `range end
  must be a scalar.` The Known bugs row closes
- **`varargin` with no extra inputs is 0x0** (S10), as today; the Known
  deviations row of cycle 07 loses that clause
- **`func2str` drops the space after the parameter list** (S11), as today:
  `@(x) x.^2+7` reads back `@(x)x.^2+7`; the Known deviations row of cycle
  06 narrows to what the page does not show, the parentheses of `@(x) (x)`
- **`any` ignores a `NaN` along a dimension of size 1** (S7), as today:
  `any(NaN, 1)` is false and `all(NaN, 1)` true. The Known deviations row
  on `any` and `all` is removed

**Not settled; the rows stay, each citing its page:**

- `1:NaN` (S1 names no rule for a `NaN` operand)
- `for` over an array with no rows (S2: `numel(valArray(1,:))` indexes a
  row a 0-by-n array does not have, and "a maximum of `n` times" allows
  fewer than `n`)
- Command syntax and an implicit `ans` in a script (S12)
- The N-D display edges, growth and deletion through fewer subscripts than
  dimensions, and a colon over an empty target (S13)
- A `-0` along a dimension of size 1 for `cumsum` (S6)
- `exist` of a local function (S14); the `det` digits (no page states
  them)
- `strcmp` of a cell holding a char array of several rows, which S9
  neither refuses nor defines

**Settled by a page, recorded for a later cycle:**

- `\` of a nearly singular matrix warns `Matrix is close to singular or
  badly scaled. Results may be inaccurate. RCOND = ...` (S15), which needs
  a condition estimate SplatCrab does not compute; the cycle 08 row keeps
  the deviation, no longer verify first
- `eig` of nonfinite input returns `NaN` values (S16), where SplatCrab
  refuses it; the page gives no shape for a two-output call, so the cycle
  08 row keeps the deviation, the one-output answer settled and the
  two-output answer still verify first

**The tables and the docs.** In `docs/ARCHITECTURE.md`: the Known bugs
rows for the colon operand, the string functions and `isequal` of handles
are removed; the rows for `1:NaN`, `for` over no rows and command syntax
cite S1, S2 and S12 as not settling them, and the closing paragraph on the
verify-first rows says so; the Known deviations row on `any` and `all` is
removed; the `-0` row narrows to `cumsum`; cycle 07's row loses its
transposition, `isequal` and `varargin` clauses; cycle 06's row narrows its
`func2str` clause; cycle 08's row records S15 and S16. `docs/HANDBOOK.md`
drops its statements that a cell cannot be transposed and that `isequal`
of cells or structs is false, and shows both working. `README.md` and
`docs/FEATURES.md` gain the rows for each change

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.
- Every behaviour recorded above for a later cycle: the nearly singular
  warning and its condition estimate, and `eig` of nonfinite input.
- The rows no page settles, which stay as they are.
- `iscellstr`, `isequaln`, `colon` as a function, nested functions, and
  `==` of handles.
- N-D cells and structs.

## Design notes

Recorded as decisions are made. Start from the
"Key designs to preserve" section of docs/ARCHITECTURE.md and record here:
which files and types changed, which invariants were preserved (column-major
storage, 1-based to 0-based conversion at the index boundary, the `end` stack,
name resolution order, the output sink), and any deviation from this spec
that was accepted deliberately.

Settled at planning:

- **One behaviour per row.** Each settled row changes at most one
  behaviour, and a row a page settles only in part keeps the part it does
  not settle, so every change here traces to a quoted sentence.
- **Two named handles are the same function** when they bind the same
  thing where they were made: both no local function, which resolves the
  name when called, or the same local function of the same file. An
  anonymous handle is equal to its copies because a copy is the same
  handle; the comparison is by identity, not by the text or the captures,
  since S4 makes two handles of the same text unequal.
- **A refusal, not a new answer, for a char array of several rows** where
  a page takes a character vector: S8 says such input "result[s] in an
  error" for most text functions, and every place listed already refuses a
  value that is not text with its own message, so the same message covers
  the new case and no message is added. `strtrim` is the exception its own
  page makes.
- **The `-0` case outside this cycle's folder.** Cycle 14b's
  `reduce_2d_past_ndims_unchanged` asserts `1/sum(y, 1)` of `y = -0` is
  `Inf`, the answer the Known deviations row recorded against S6's "or when
  `size(A,dim)` is `1`"; that line becomes `-Inf` and the case's covers
  line says so, since S6 now settles it.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/15-verify-first/`, as a `.m` case unless it says otherwise.
Expected output follows from this spec's rules and the existing display:
`disp` of a logical scalar is `   1`, of a logical row `   0   0   1`, of a
double `-Inf` `  -Inf`.

1. `isequal` of handles: `fun1 = @sin; fun2 = @sin; disp(isequal(fun1,
   fun2))` → `   1`; `A = 5; h1 = @(x) A * x.^2; h2 = @(x) A * x.^2;
   disp(isequal(h1, h2))` → `   0`; then `h2 = h1; disp(isequal(h1, h2))`
   → `   1`; `isequal(@sin, @cos)` → `   0`; `isequal(str2func('sin'),
   @sin)` → `   1`; `isequal(@sin, @(x) sin(x))` → `   0`; `isequal(@sin,
   1)` → `   0`; `c = {h1}; isequal(c{1}, h1)` → `   1`; `isequal(h1, h1,
   h1)` → `   1`; two handles to one local function of the script,
   `isequal(@loc, @loc)` → `   1`.
2. `isequal` of cells: `isequal({1, 'a'}, {1, 'a'})` → `   1`; `{1, 'a'}`
   against `{1, 'b'}` → `   0`; `{1, 2}` against `{1; 2}` → `   0`;
   `{'a'}` against `{97}` → `   1`; `{NaN}` against `{NaN}` → `   0`;
   `isequal({}, {})` → `   1`; `{{1, {2}}}` against `{{1, {2}}}` →
   `   1`; `isequal({1}, 1)` → `   0`; `isequal({@sin}, {@sin})` → `   1`;
   `isequal({1, 2}, {1, 2}, {1, 2})` → `   1`.
3. `isequal` of structs: `s.a = 1; s.b = 'x'; t.b = 'x'; t.a = 1;
   isequal(s, t)` → `   1`; then `t.b = 'y'` → `   0`; `u.a = 1;
   isequal(s, u)` → `   0`; `p(1).a = 1; p(2).a = 2; q = p;
   isequal(p, q)` → `   1`; then `q(2).a = 3` → `   0`; `isequal(s,
   {1})` → `   0`.
4. Deep nesting: two cells each nested 100,000 deep around `1`,
   `c = 1; d = 1; for k = 1:100000, c = {c}; d = {d}; end`, are equal,
   `   1`; around `1` and `2` unequal, `   0`; a struct nested 100,000 deep
   through a field alike, `   1` and `   0`; exit 0 in each case.
5. Transposition: `c = {1, 'ab', [3 4]}; d = c'; disp(size(d)); disp(d{2});
   disp(d{3})` → `     3     1`, `ab`, `     3     4`; `isequal(c', c.')`
   → `   1`; `disp(size(transpose(c)))` → `     3     1`; `g = {1, 2, 3;
   4, 5, 6}'; disp(size(g)); disp(g{1, 2}); disp(g{3, 1})` → `     3     2`,
   `     4`, `     3`; `z = {1+2i}'; disp(imag(z{1}))` → `     2`;
   `s(1).a = 1; s(2).a = 2; t = s'; disp(size(t)); disp(t(2).a)` →
   `     2     1`, `     2`, and `s.'` alike. `err_*`: `f = @sin; f'` →
   `This operation is not supported for a value of class
   'function_handle'.`, exit 1.
6. `sum` and `mean` along a dimension of size 1: `disp(1/sum(-0))`,
   `disp(1/sum(-0, 1))`, `disp(1/mean(-0))`, `disp(1/mean(-0, 1))` →
   `  -Inf` each; `disp(1 ./ sum([-0 -0], 1))` → `  -Inf  -Inf`;
   `disp(1 ./ mean([-0; -0], 2))` → `  -Inf` twice; `x = 1 ./ sum(-0 *
   ones(1, 1, 2), 1); disp(x(:)')` → `  -Inf  -Inf`; unchanged:
   `disp(1/sum(-0, 3))` → `  -Inf`, `disp(1/cumsum(-0, 1))` → `   Inf`,
   `disp(class(sum(true, 1)))` → `double`.
7. `any` and `all`: `disp(any(NaN, 3))` → `   0`; `disp(all(NaN, 3))` →
   `   1`; `disp(any([0 NaN 2], 3))` → `   0   0   1`; `disp(all([0 NaN
   2], 3))` → `   0   1   1`; `disp(class(any(NaN, 3)))` → `logical`;
   `disp(size(all(NaN(2, 3), 5)))` → `     2     3`; unchanged:
   `disp(any(NaN, 1))` → `   0`, `disp(all(NaN, 1))` → `   1`.
8. Text functions refuse a char array of several rows, each message
   printed through `try` and `catch`: `upper({['ab'; 'cd']})` and
   `lower({'ab'.'})` → `Every element of a cell argument to 'upper' must
   be a character vector.` and its `lower` form; `strrep(['ab'; 'cd'],
   'a', 'z')` → `Argument 1 to 'strrep' must be a character vector.`;
   `strrep('abc', ['a'; 'b'], 'z')` → `Argument 2 to 'strrep' ...`;
   `strrep({['ab'; 'cd']}, 'a', 'z')` → `Every element of a cell argument
   to 'strrep' ...`; `strfind(['ab'; 'cd'], 'a')` → `Argument 1 to
   'strfind' ...`; `regexp(['ab'; 'cd'], 'a', 'match')` → `Argument 1 to
   'regexp' ...`; `regexprep('abc', ['a'; 'b'], 'z')` → `Argument 2 to
   'regexprep' ...`; `regexprep({['ab'; 'cd']}, 'a', 'z')` → `Every
   element of a cell argument to 'regexprep' ...`; `strjoin({'a', ['b';
   'c']})` → `Every element of a cell argument to 'strjoin' ...`;
   `strjoin({'a', 'b'}, ['-'; '+'])` → `Argument 2 to 'strjoin' ...`;
   `strcat({['ab'; 'cd']}, 'z')` → `Every element of a cell argument to
   'strcat' ...`; `strsplit(['a b'; 'c d'])` → `Argument 1 to 'strsplit'
   ...`; `strtok(['a b'; 'c d'])` → `Argument 1 to 'strtok' ...`;
   `upper({cat(3, 'a', 'b')})` → `Every element of a cell argument to
   'upper' ...`; `strtrim({cat(3, 'a', 'b')})` → `N-D arrays are not
   supported by 'strtrim'.` Beside it, `err_*` cases for `upper` of a cell
   and `strrep` of a char matrix, each exit 1.
9. Text functions keep what they took: `x = upper(['ab'; 'cd']); disp(x)`
   → `AB` and `CD`; `r = strtrim({[' ab'; ' cd']}); disp(size(r{1}));
   disp(r{1})` → `     2     2`, `ab`, `cd`; `r = strtrim({['ab '; ' cd']});
   disp(size(r{1}))` → `     2     3`; `disp(strcat(['a'; 'b'], ['x';
   'y']))` → `ax` and `by`; `disp(numel(upper({char(zeros(0, 3))})))` →
   `     1`; `x = upper({'ab', ''}); disp(x{1}); disp(isempty(x{2}))` →
   `AB` and `   1`.
10. The colon: through `try` and `catch`, `[1 3]:4` and `[1 2 3]:2:10` →
    `range start must be a scalar.`; `1:[1 2]:5` → `range step must be a
    scalar.`; `1:[3 4]`, `1:[]` and `1:size(ones(3, 4))` → `range end must
    be a scalar.`; `for k = 1:[], disp(k), end` → `range end must be a
    scalar.`; and `err_*` cases for a start, a step and an end operand,
    each exit 1.
11. `varargin`: the S10 function `definedAndVariableNumInputs(X, Y,
    varargin)`, printing `fprintf('Size of varargin cell array: %dx%d\n',
    size(varargin))`, called with two inputs and with five → `Size of
    varargin cell array: 0x0` and `Size of varargin cell array: 1x3`.
12. `func2str`: `disp(func2str(@(x) x.^2+7))` → `@(x)x.^2+7`; `fh =
    @(x,y)sqrt(x.^2+y.^2); disp(['Anonymous function: ' func2str(fh)])` →
    `Anonymous function: @(x,y)sqrt(x.^2+y.^2)`; `disp(func2str(@cos))` →
    `cos`.
13. Unit tests: handle equality by name and binding and by identity;
    cells and structs compared element by element, fields in any order,
    and a deep nesting compared without recursion; the transposition of a
    cell and a struct array's elements; `sum` and `mean` along a dimension
    of size 1 keeping a `-0`; `any` and `all` past `ndims` with a `NaN`;
    the character-vector test at each place in the text functions. Tests:
    `named_handles_are_equal_when_they_bind_the_same_function`,
    `anonymous_handles_are_equal_only_to_their_copies`,
    `cells_and_structs_compare_every_element`,
    `struct_fields_compare_in_any_order`,
    `a_deep_nesting_compares_without_recursion`,
    `a_cell_and_a_struct_array_transpose`,
    `sum_and_mean_along_a_dimension_of_size_one_return_the_argument`,
    `any_and_all_past_ndims_take_a_nan`,
    `a_char_of_several_rows_is_no_character_vector`,
    `strtrim_trims_a_char_matrix_in_a_cell_as_on_its_own`.
14. Every existing case passes unchanged, apart from the one line of
    `14b-nd-functions/reduce_2d_past_ndims_unchanged` that S6 settles and
    its covers line. Cases: the whole golden suite.

## Status

Planned
