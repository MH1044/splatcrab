# 03 — Indexing forms

## Goal

logical indexing read/write, deletion `x(i) = []` (row-vector rule, "only one non-colon index"), in-place `assign_index` (validate-then-mutate, no clone), shared `resolve_read/resolve_write/gather`, lexer `{ } . @`, `Expr::Access(name, Vec<Access>)` + `LValue`, `Stmt::MultiAssign` with `~`, `nargout`-aware `max min sort size find`

After this module a user can do things that were not possible before; every
bullet in Scope must be demonstrable by at least one acceptance test below.

## Scope

- logical indexing read/write
- deletion `x(i) = []` (row-vector rule, "only one non-colon index")
- in-place `assign_index` (validate-then-mutate, no clone)
- shared `resolve_read/resolve_write/gather`
- lexer `{ } . @`
- `Expr::Access(name, Vec<Access>)` + `LValue`
- `Stmt::MultiAssign` with `~`
- `nargout`-aware `max min sort size find`
- Trailing singleton subscripts (QA D22): `A(2, 1, 1)`, `A(:, :, 1)` and
  `A(1, 2, 1) = 9` work, with `end` equal to 1 in a third position, and a
  third index past 1 is the usual "Index in position 3 exceeds array bounds"
  error
- A logical mask is a mask, never a list of indices, including a mask with
  no zeros (QA D6): `x(x > 0)` on `[5 6 7]` is `5 6 7`, not `5 5 5`
- The size-overflow message for indexed growth names the size asked for:
  `x = []; x(1e300) = 1` reports `1x1e+300`, not `usize::MAX`. Keep the
  requested size as `f64` and judge it with `args::check_shape`, whose
  `fmt_dim` formatting cycle 01c added for constructors and the colon;
  `check_size`, which growth reaches today, renders the saturated `usize`

- A logical mask behaves as `find(mask)` would, which fixes the result's
  shape: a mask indexing a vector gives that vector's orientation, and a
  matrix mask indexing a matrix gives a column. A mask shorter than the array
  selects only among the elements it covers; a mask with a `true` past the
  end is an index-out-of-bounds error on read and grows the array on
  assignment, as a numeric index would
- The invalid-index message gains MATLAB's ending, now that logical values
  are true of this interpreter: `Index in position 1 is invalid. Array
  indices must be positive integers or logical values.` This discharges the
  Known deviations row that scheduled it here
- Cycle 02's stand-in is removed: the `Logical indexing is not supported
  yet.` message leaves `src/error.rs`, and its case
  `02-classes-and-display/err_logical_index` is deleted, as that case's own
  comment says this cycle would do. Item 16 below takes its place
- Brace and dot access on a matrix parse (they are the access-chain AST
  above) and are clean errors at run time, with MATLAB's own texts:
  `Brace indexing is not supported for variables of this type.` and `Dot
  indexing is not supported for variables of this type.` Both stay correct
  for a matrix after cycle 07 adds the containers that do support them

Added at this cycle's planning, from the Known deviations and Known bugs
tables and from what cycle 02 left behind. None of it widens the Goal.

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.
- Evaluating brace, field and dynamic-field access at runtime. This cycle
  lands the access-chain AST and lexer tokens only; cycles 06 and 07 use them.

## Design notes

### Files and types changed

- `src/lexer.rs`: `Token::LBrace`, `RBrace`, `Dot` and `At`, each with its
  `Display` spelling. The field dot is whatever `.` remains after a number's
  decimal point, the five dotted operators and a `...` continuation have been
  matched, so the older dotted forms lex exactly as before (unit tests cover
  `1.5`, `.5`, `2.5.^x`, `x.^2`, `x.*y`, `x./y`, `x.\y`, `x.'` and
  `a...`). `RBrace` ends a value, so `c{1}'` is a transpose. Braces go on the
  delimiter stack; `{` and `@` after whitespace inside brackets start a new
  element.
- `src/parser.rs`: `Expr::Index(String, Vec<Expr>)` is replaced by
  `Expr::Access(String, Vec<Access>)`, with `Access::{Paren, Brace, Field,
  DynField}`; the chain is never empty, a bare name stays `Expr::Ident`. The
  `LValue { name, chain }` type is every assignment target, so
  `Stmt::Assign(String, ..)` and `Stmt::IndexAssign` became one
  `Stmt::Assign(LValue, Expr, bool)`. `Stmt::MultiAssign(Vec<Option<LValue>>,
  Expr, bool)` is new, `None` being a `~`. `parse_chain` reads the links,
  `parse_args` an argument list with `end` and `:` enabled (a bare `:` may now
  be followed by `}`), and `try_targets` recognises a target list.
- `src/interp.rs`: `Sel` is `All` or `List { idx, rows, cols, max }`, where
  `max` is the largest one-based position as an `f64`. New: `call_form`,
  `eval_outputs`, `eval_access`, `apply_access`, `index_var`, `assign_to`,
  `target_mut`, `delete_index`, and the free functions `mask_positions`,
  `index_positions`, `check_trailing`, `resolve_read` + `gather`,
  `resolve_write` + `scatter`, `resolve_delete`, with the plan types
  `Gather`, `Scatter` and `Keep`. `index`, `index_read` and `index_values`
  are gone.
- `src/error.rs`: `null_assignment_indices`, `brace_indexing_unsupported`,
  `dot_indexing_unsupported` and `insufficient_outputs` are new;
  `index_not_positive_integer` gains "or logical values."; the stand-ins
  `logical_indexing_unsupported` and `deletion_unsupported` are removed.
- `src/builtins/`: `max` and `min` (`math.rs`, with the new `arg_extremum`),
  `sort` and `find` (`linalg.rs`) and `size` (`core.rs`) read `nargout`.
  `args::check_size` is removed: indexed growth was its only caller, and
  growth now goes through `check_shape`.
- `src/main.rs`: `needs_more` counts `{ }` with the parentheses, so an `end`
  inside a brace index does not close a REPL block.

### Invariants preserved

- Column-major storage: every resolver works in linear column-major
  positions; `scatter` keeps the layout when growth only lengthens the last
  dimension and re-lays the columns otherwise.
- One-based to zero-based at one boundary: `eval_index_args`, through its two
  helpers `index_positions` (numeric and char subscripts) and
  `mask_positions` (logical ones), is the only place a subscript is
  converted, for reading, assignment and deletion alike. The function kept
  its name, so invariant 2 in `docs/ARCHITECTURE.md` still
  names it; ARCHITECTURE now also names the helpers.
- The `end` stack: pushed per subscript in `eval_index_args` only, with the
  element count for one subscript, rows and columns in the first two
  positions of several, and `1` in any later position. A call still never
  pushes.
- Name resolution: variable first, then builtin. `call_form` is the single
  test of "this is a call", shared by statements (nargout 0) and
  `MultiAssign` (nargout n).
- The output sink: `MultiAssign` shows each output through `Interp::emit`.
- Errors are values: no new panic path. `target_mut` has an `unreachable!`
  after an insert of the same key, which cannot fire.

### Choices where the spec is silent

- **The deletion form** is the literal `[]` on the right-hand side of an
  assignment whose target is one `(...)`: `Expr::Matrix` with no rows, so
  `[ ]` and `[;]` count too. An empty that arrives as a value (`e = [];
  x(2) = e`) is an ordinary assignment and fails the element count, as it
  does in MATLAB. `x(2) = ''` is also an assignment, not a deletion
  (unverified against MATLAB).
- **Deletion shapes.** One subscript: a column vector stays a column, every
  other array (a row, a scalar, a matrix) leaves a row, and `x(:) = []`
  leaves a 0x0. Two or more subscripts: a subscript that selects every
  position of its dimension, in any order (`:`, `1:end`, `[2 1]` of two
  rows), counts as a colon; more than one that does not is the null
  assignment error. When both select everything, the one written as a list
  deletes (`A(1:end, [1 2]) = []` removes the columns), and `A(:, :) = []`
  removes every row, leaving `0 x cols` (unverified). Subscripts past the
  second must select position 1 and never delete. A deletion that removes
  nothing, `A([]) = []`, leaves the array exactly as it was, shape included.
  A position past the end is the ordinary bounds error
  (`index_exceeds_numel` or `index_exceeds_bound`), not a deletion-specific
  message.
- **Mask shapes** follow `find`'s rule exactly: a row mask gives a row, a
  0x0 mask a 0x0, anything else a column; the result is then shaped as a
  numeric index of that shape would shape it. A mask with no `true` is an
  empty index (`x(x > 9)` of a row is 1x0).
- **Trailing subscripts** past the second must each select position 1
  exactly once. A position past 1 is the bounds error on read; selecting
  position 1 twice or not at all (`A(:, :, [1 1])`, `A(:, :, [])`), or
  assigning past page 1 (`A(1, 1, 2) = 5`), would need an N-D array and is
  the existing `N-D arrays are not supported.` MATLAB builds the N-D array;
  this is the same deviation as the constructors' N-D row in Known bugs.
- **`x()`** with no subscripts is still `Only 1-D and 2-D indexing is
  supported.` MATLAB returns `x`; this cycle does not change it.
- **Chains on a matrix.** A second `(...)` indexes the value so far, so
  `x(2:3)(2)` and `size(A)(2)` work, as in Octave; MATLAB refuses to chain
  parentheses. The first brace or field link in a chain, reading or
  assigning, is the matching Brace/Dot message, and a brace or field on a
  builtin's value (`pi.a`) is the same. `x(1)(2) = v` is the existing
  `invalid assignment target`. `s.a = 1` for an undefined `s` is the Dot
  message rather than the struct MATLAB creates, until cycle 07.
- **Multiple assignment.** The right-hand side is a call when it is a name
  that is not a variable, bare or with one `(...)`; it is asked for one value
  per target, `~` included, and must return at least that many, else
  `Too many output arguments.` Anything else is evaluated first (so its own
  error comes first) and is one value: enough for `[x] = ...`, and
  `Insufficient number of outputs ...` for more targets. Outputs are assigned
  and shown one at a time in the order written, and `ans` is not set. A
  bracket is a target list only when it holds nothing but names (with
  chains) and lone `~`s and is followed by `=`; `[a ~ c] = f(x)` is therefore
  the matrix `[a, ~c]` and an invalid target, as in a literal.
- **The second output of `max` and `min`** is a double index along the
  reduced dimension: the first of a tie, `NaN` skipped, `1` for an all-`NaN`
  slice; a linear index with `'all'`; ones along a dimension past the
  array's; an empty of the value's shape for an empty. The two-array form
  returns one value, so asking it for two is `Too many output arguments.`
  (MATLAB has its own message for that; not recorded, so not used).
- **`[s, i] = sort(...)`** sorts positions keyed by value, stably, so the
  permutation and the values agree by construction; along a dimension the
  vector does not extend in, every index is 1.
- **`size` with several outputs** gives rows, columns, then `1` for every
  further output; one output (or none, at statement level) is the size row;
  `size(A, dim)` returns one value whatever is asked.
- **`find` with several outputs** gives rows and columns, and with three the
  values in the argument's class, all in the one-output shape; the count and
  direction arguments still apply.
- **Growth** of a 0x0 or of any empty through one subscript makes a row, as
  before; `check_shape` judges `(1, need)`, `(need, 1)` or the two-subscript
  `(rows, cols)` as `f64`s, so every oversized request is named as asked.

### Accepted deviations

- `A(:, :, [1 1])`, `A(:, :, [])` and growth into a second page are errors
  rather than N-D results (no N-D arrays yet).
- `x(1)(2)` reads where MATLAB refuses; `x()` refuses where MATLAB returns
  `x`.
- The deletion out-of-range message and the two-array `[m, i] = max(a, b)`
  message are this interpreter's existing texts, not MATLAB's.
- `[a, b] = 5` and `[a, b] = x` are refused with MATLAB's text; comma-
  separated lists, which make the second form legal for cells, are cycle
  07's.

### Item 13

Measured on the development machine (Windows), whole process including
start-up, three runs each: the debug binary the golden harness runs takes
about 1.2 s, the release binary about 0.33 s. The growth itself is linear:
doubling the count doubles the time, and a unit test checks that 10,000
appends reallocate fewer than 64 times. The loop overhead dominates: the same
loop with `z = k + 1` in place of `z(end+1) = k` takes about 0.8 s in debug
and 0.26 s in release. "Well under one second" therefore holds in release
and not in the unoptimised debug build; making the debug build meet it is an
interpreter-wide speed question (or a `[profile.dev]` opt-level), not an
indexing one.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/03-indexing-forms/`. Expected output is written by hand from MATLAB
semantics wherever feasible; the `.err` substring is always chosen by hand.
Every new error message needs an `err_*` case.

1. `x = [5 3 8 1]; disp(x(x > 2))` → `     5     3     8` Cases: logical_mask_read.
2. `x = 1:6; x(x > 4) = 0; disp(x)` → `     1     2     3     4     0     0` Cases: logical_mask_assign.
3. `A = [1 2 3; 4 5 6; 7 8 9]; disp(A(A > 5)')` → `     7     8     6     9` Cases: logical_mask_matrix_column.
4. `x = 1:5; x(2) = []; disp(x); x(logical([1 0 1 0])) = []; disp(x)` → `     1     3     4     5\n     3     5` Cases: delete_vector_elements, delete_shows_result.
5. `A = [1 2 3; 4 5 6]; A(:, 2) = []; disp(A); A(1, :) = []; disp(A)` → `     1     3\n     4     6\n     4     6` Cases: delete_column_and_row.
6. `A = [1 2; 3 4]; A(2) = []; disp(size(A))` → `     1     3`; `A = [1 2; 3 4]; A(1, 2) = []` → err `A null assignment can have only one non-colon index.` Cases: delete_linear_from_matrix, err_delete_two_indices.
7. `[m, i] = max([3 9 2])` → `m =\n\n     9\n\ni =\n\n     2\n` Cases: multi_assign_max_display.
8. `[r, c] = size(zeros(2, 5)); fprintf('%d %d\n', r, c); [~, i] = min([4 2 8]); disp(i); [s, idx] = sort([3 1 2]); disp(idx)` → `2 5\n     2\n     2     3     1` Cases: multi_assign_size, multi_assign_tilde_min, multi_assign_sort.
9. `[r, c] = find([0 1; 1 0]); disp([r c])` → `     2     1\n     1     2` Cases: multi_assign_find.
10. `A = zeros(2); A(:) = 1:4; disp(A); x = []; x(3) = 1; disp(x)` → `     1     3\n     2     4\n     0     0     1` Cases: assign_colon_fills, grow_from_empty.
11. `x = 1:5; x(end+1) = 6; x(end) = []; disp(numel(x)); x(0)` → `     5` then err `Index in position 1 is invalid. Array indices must be positive integers or logical values.` Cases: err_index_zero_ending.
12. `[a, b] = 5` → err `Insufficient number of outputs from right hand side of equal sign to satisfy assignment.`; `[a, b] = sum([1 2])` → err `Too many output arguments.` Cases: err_multi_assign_insufficient, err_multi_assign_too_many.
13. Perf guard: `z = []; for k = 1:200000, z(end+1) = k; end; disp(numel(z))` → `    200000` well under one second. Cases: growth_perf_guard, repl_failed_assign_unchanged. Two notes from the close of the cycle: the recorded `    200000` predates cycle 02's integer-width rule, which displays it 12 wide, so the case asserts the count with `%d` instead; and the loop takes about 0.33 s in a release build but about 1.2 s in the debug build the golden harness runs, where the cost is the interpreter's per-statement overhead rather than growth, which a unit test pins as amortised.
14. Parser unit tests: `x{2}`, `s.a`, `s.(n)`, `c{1}(2).b` produce the expected `Access` chains; `[a, ~, c] = f(x)` parses to `MultiAssign`. The `@` token has a golden case besides its unit tests, because its parse message changed: Cases: err_at_sign_parse.
15. `A = [1 2; 3 4]; disp(A(2, 1, 1)); disp(A(:, :, 1)); A(1, 2, 1) = 9; disp(A)` → `     3\n     1     2\n     3     4\n     1     9\n     3     4`; `A(1, 1, 2)` → err `Index in position 3 exceeds array bounds. Index must not exceed 1.` Cases: trailing_singleton_subscripts, trailing_singleton_end, err_trailing_singleton_bound.
16. `x = [5 6 7]; disp(x(x > 0)); x(x > 0) = 0; disp(x)` → `     5     6     7\n     0     0     0` Cases: logical_mask_all_true.
17. `x = []; x(1e300) = 1` → err containing `Requested 1x1e+300 array`, exit code 1. Cases: err_growth_size_named.
18. `x = [1 2]; x{1}` → err `Brace indexing is not supported for variables of this type.`; `x = [1 2]; x.a` → err `Dot indexing is not supported for variables of this type.`; both exit 1 Cases: err_brace_on_matrix, err_dot_on_matrix.
19. `x = [10 20 30]; disp(x(logical([1 0]))); A = [1 2; 3 4]; disp(A(logical([1 0 0 1]))); disp(A(logical([1 0; 0 1]))); y = [1 2]; y(logical([0 0 1])) = 9; disp(y)` → `    10
     1     4
     1
     4
     1     2     9` Cases: mask_shorter_than_array, mask_find_shape, mask_assign_grows, err_mask_true_past_end.

## Status

Done (2026-09-28)
