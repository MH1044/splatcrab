# 07 — Cells and structs

## Goal

`CellArray`, `StructArray`, brace/field/dynamic-field read and write via `assign_chain`, cs-lists, `for` over cells, `cell struct fieldnames isfield rmfield getfield setfield iscell isstruct cellfun num2cell cell2mat deal`, `varargin/varargout`, displays

After this module a user can do things that were not possible before; every
bullet in Scope must be demonstrable by at least one acceptance test below.

## Scope

- `CellArray`
- `StructArray`
- brace/field/dynamic-field read and write via `assign_chain`
- cs-lists
- `for` over cells
- `cell struct fieldnames isfield rmfield getfield setfield iscell isstruct cellfun num2cell cell2mat deal`
- `varargin/varargout`
- `e.stack` of a caught `MException`, a struct array of `file`, `name` and
  `line`, one element per frame. Cycle 04 deferred it to 05 and 05 to
  here: it is a struct array, and structs are this cycle's. Cycle 05's
  error trace is the data it holds
- displays
- **What earlier cycles deferred here.** `arrayfun(..., 'UniformOutput',
  false)`, moved from cycle 06, returning a cell. `cellfun` and `arrayfun`
  call through `Interp::call_nested`, cycle 05's rule for every builtin
  that calls back into the interpreter. And the Known bugs row "most
  builtins refuse an `MException`", which cycle 06 widened to handles:
  `size`, `numel`, `isempty`, `isa`, `class` and the `is*` predicates
  answer for every value this interpreter has (a handle and an
  `MException` are 1x1, never empty, and `isa(e, 'MException')` is true)
- **Operators on a value that is not an array** use MATLAB's R2020a text,
  per cycle 01e's message policy: `Operator '+' is not supported for
  operands of type 'cell'.`, and the same sentence with the operator and
  the class for a struct, a function handle and an `MException`. The
  sentence is confirmed by MathWorks Answers thread titles for `cell` and
  `function_handle`. Cycle 04's `This operation is not supported for a
  value of class 'MException'.` gives way to it for binary operators, so
  `04-switch-try-commands/err_exception_arithmetic.err` changes for that
  reason; unary operators and other operations keep SplatCrab's generic
  text, whose MATLAB wording no source here settles
- **Freeing never recurses.** A cell can hold a handle, and a handle can
  capture a cell, so a long chain through cells must be freed from
  `Func`'s drop worklist (cycle 06's review fix), extended to cells and
  structs, or the stack overflow that fix closed comes back through the
  new value kinds. The same holds for deeply nested cells and structs
  built in a loop, `c = {c}` 500,000 times
- The protocol's `workspace` lists cells and structs with their class and
  size; U0's `vars` shape is unchanged. Handles inside cells,
  `{@(x) x + 1, 2}`, which cycle 06 lexed and parsed, now evaluate

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.
- `isequal` of handles, and struct arrays with fields of different shapes
  in one display: SplatCrab's own until a source settles them.


## Design notes

Filled in as decisions are made. Start from the
"Key designs to preserve" section of docs/ARCHITECTURE.md and record here:
which files and types changed, which invariants were preserved (column-major
storage, 1-based to 0-based conversion at the index boundary, the `end` stack,
name resolution order, the output sink), and any deviation from MATLAB that
was accepted deliberately.

### Files and types

- `value.rs`: `Value::Cell(Rc<CellArray>)` and `Value::Struct(Rc<StructArray>)`,
  column-major like `Matrix` (a struct array's element `k` is one value per
  field, in the order the fields were first made). `blank()` is the `[]` a
  new element, field or struct element holds. The displays. `holds_values`
  and `free`, the drop worklist cycle 06 built for `Func`, now shared by
  `Drop` for `CellArray`, `StructArray` and `Func`.
- `lexer.rs`: a `{` that does not follow the end of a value is a literal
  brace (`C` on the delimiter stack), with a bracket's whitespace and
  newline rules; a brace index is unchanged.
- `parser.rs`: `Expr::Cell(rows)`; `parse_cell` wants a separator after each
  element, so `{@(x) x 1}` is refused; `render` writes braces, so `func2str`
  of a body holding a cell reads back.
- `interp.rs`: the cs-list read path (`eval_access` returns `Vec<Value>`,
  `eval_multi`, `one_value`, `Got` in `eval_request`), the write path
  (`resolve_links`, `shape_at`, `assign_chain`, `assign_paren`,
  `assign_matrix`, `delete_in`, `nav_mut`), container concatenation in
  `hcat`/`vcat`, `for` over containers, `varargin`/`varargout` in
  `call_user` and `call_anon`, `e.stack`, and binary operators' refusal.
  `resolve_write` takes the right-hand side's shape rather than a matrix, so
  cells and struct arrays plan through it. `Unit.file` records a path
  file's path for `e.stack`.
- `builtins/cells.rs`: the thirteen builtins and `map_elements`, which
  `arrayfun` (still registered in `core.rs`) and `cellfun` share. `core.rs`:
  the shape and class queries on `Value::dims` and the class, for every value.
- `error.rs`: the messages below, and `StackEntry.file` with
  `MError::leaving_file`.

### Invariants

Column-major storage holds for both containers; growth, reads and deletion
reuse `resolve_read`/`resolve_write`/`resolve_delete`, and the only
one-based to zero-based conversion is still `eval_index_args` (container
subscripts go through it, brace or paren, read or write). `end` in an
assignment target is pushed with the shape of what the path holds at that
link, which `shape_at` finds by reference, `0x0` where nothing is yet. Name
resolution and the output sink are untouched.

### Rules chosen where the spec is silent

- **Display.** A cell element is summarised on one line: a numeric or
  logical scalar `{[1]}`, padded inside the brackets so numbers
  right-align; a char row `{'ab'}`; a handle its text; anything else
  `{r×c class}`, padded before the closing brace; columns are four apart,
  each as wide as its widest element, wrapping into `Columns N through M`
  blocks at 80 characters. A cell inside a cell is `{1×1 cell}` and is never
  expanded, which is the bound on displaying a deep nest. A struct's field
  lines right-align the names; a value is a scalar, `[1 2 3]` when a row
  fits, `'text'`, `[]`, a handle's text, `{r×c cell}`, or `[r×c class]`
  (`[1×1 struct]` for a struct, never expanded). A struct array lists its
  field names; `0×1 empty struct array with fields:` for an empty one; a
  struct with no fields is `struct with no fields.` A struct's first line is
  `s = `, with the space the spec records. `disp` of a cell prints its rows,
  of a scalar struct its field lines, of a struct array its header.
- **`struct` with a cell value**: a cell that is not 1x1 makes a struct
  array of its size, element `k` taking element `k`; a 1x1 cell is the one
  value of every element (`struct('a', {{1, 2}})` holds the cell); every
  non-1x1 cell must have one size; `struct('a', {})` is 0x0 with the field.
  `struct()` is 1x1 with no fields and `struct(s)` is `s`. A field named
  twice keeps its first place and the later value.
- **cs-lists.** Spread into call arguments, `[ ]` and `{ }`; `[a, b] = c{:}`
  assigns in order and too few is `Insufficient number of outputs ...`; as a
  statement each value is shown as `ans`; anywhere else, one value is
  wanted, and a list of any other length, `0` included, is the spec's
  message with the count. A cs-list in the middle of a chain,
  `p.name(1)` of a 1x2 `p`, is the same message. A brace assignment or a
  `p(k).f = v` that selects more than one element is the same message too.
  Not supported: a cs-list inside index subscripts, `x(c{:})`, and a
  cs-list target list, `[c{:}] = deal(0)`.
- **`nargin`/`nargout` with `varargin`/`varargout`**: both count every
  argument and every output asked for, those in `varargin` and `varargout`
  included (MATLAB's rule inside the function). `varargin` with no extra
  arguments is the 0x0 cell `{}` (unverified: MATLAB may give 1x0).
  Asked for no outputs, a function whose only output is `varargout` gives
  `varargout{1}` if set, which becomes `ans`. An anonymous function also
  takes a last parameter `varargin`.
- **`cellfun`** takes a handle or a function's name, called as `feval`
  calls one (so `cellfun('isempty', c)` works). The option is
  `'UniformOutput'` in any case, followed by a scalar; `'ErrorHandler'` is
  not supported and is read as an input. `arrayfun` now accepts any value as
  an input, handing out `Value::element(k)` (a 1x1 cell of a cell, a 1x1
  struct of a struct array). Uniform results must be matrix scalars; the
  output's class is the results' when they share one, else double. Every
  call goes through `call_nested`.
- **Concatenation.** With a cell among the operands, the result is a cell and
  every non-cell operand one element; `[]` and empties drop out. With a
  struct, every operand must be a struct with the same fields (in any order;
  the first operand's order is kept).
- **Assignment into containers.** `[]` (and an undefined variable) becomes
  whatever the first link needs. `c(k) = v` needs a cell `v`, a struct array
  a struct array with the same fields; a matrix refuses a cell or a struct
  with `Conversion to double from cell is not possible.`. A failed
  assignment changes nothing: a new element or field is built on its own
  and only placed once the assignment below it succeeded, and a variable
  created for the walk is removed.
- **`for`** over a cell or a struct array takes one column per iteration,
  a `rows x 1` container; over no columns it assigns the empty container.
- **`e.stack`** is an Nx1 struct array of cycle 05's trace entries,
  innermost first: `name` as the trace names it, `line` the line (`[]` for an
  anonymous function), `file` the path of a function file or path script and
  `''` for a function local to the code that was run, whose path the
  interpreter is not told. The script's own frame is not an element, so an
  error raised outside every function has a 0x1 stack.
- **Operators.** Every binary operator, `&&` and `||` included, uses the
  R2020a sentence with the first operand that is not an array; both
  operands are evaluated first. Unary operators and transpose keep the
  generic text, so `c'` of a cell is refused (MATLAB transposes a cell).
- **Freeing.** `holds_values` names cells, structs and handles; `free` pops a
  value, and when it is the last owner moves the nesting values it holds onto
  the list before dropping it, so no drop recurses more than one level. A
  deep chain is freed when cleared, reassigned or alive at exit. Nothing
  else recurses on nesting: clones are `Rc` copies, displays summarise one
  level, `isequal` of containers is false without looking inside, and
  `assign_chain` is bounded by `MAX_DEPTH` links.
- **Many fields.** `field_index` walked the names, so `s.(n) = v` for
  100,000 new names was quadratic and ran for minutes (found in testing).
  A struct of 32 fields or more now keeps a private hash index from name to
  first place, built on its first lookup and updated by `ensure_field`, the
  one place a field is added; every struct is made by `StructArray::new` or
  `scalar`, so none can be built with a stale index.
- **Many pairs in `struct(...)`** (found at review). The builtin gathered its
  names in a list it scanned for each pair, the same fault on a second path:
  `struct(c{:})` with 100,000 pairs took 81 s. Names seen are now found
  through a map. `struct_many_pairs` runs 50,000 pairs, whose old cost of
  18.9 s is past the harness's timeout, in under two seconds.

### Messages

MATLAB's, as the spec records them: the cs-list count, the dot-assignment
text and the operator sentence. MATLAB's as recalled, not confirmed:
`Unrecognized field name "b".` (a missing field, also for `rmfield`),
`Conversion to cell from double is not possible.`, `Subscripted assignment
between dissimilar structures.`, `Scalar structure required for this
assignment.`, `Invalid field name: 'a b'.`, `The number of outputs should
match the number of inputs.` (`deal`), and the brace form of the assignment
text. SplatCrab's own: `Structures being concatenated must have the same
field names.`, `A dynamic field name must be a character vector.`, `Field
names and values to 'struct' must come in pairs.`, `The cell values given to
'struct' must all have one size, or be 1x1.`, `cell2mat does not support
cells holding cells, structs, handles or MExceptions.`, `Argument 2 to
'cellfun' must be a cell array.`, `Argument 1 to 'fieldnames' must be a
struct.`, `The variable varargout must be a cell array.` and
`Output argument "varargout{3}" ...` for a missing `varargout` element.
Every one of these, recalled or own, is pinned by an `err_*` case under its
Acceptance item; the unary operators and transpose on a container keep
cycle 04's generic text, which is not new.

### Deviations accepted

The unsupported forms above (`x(c{:})`, `[c{:}] = ...`, `c'`, `isequal` of
containers, `'ErrorHandler'`), `varargin` being 0x0 when empty, `e.stack`
without the script's frame, and the recalled message texts. Recorded in
the Known deviations table of `docs/ARCHITECTURE.md`.

### Settled in testing

The bytes the cases flagged, each settled from the spec's rules:

- **`Error: Line N: `.** The prefix is the existing cases' form; N is the
  script's own statement, counting the `% covers:` line as line 1, and for
  an error raised inside a function or at a call's output check it is the
  script's line of the call (cycle 05's rule), as in
  `err_cellfun_recursion_limit` and `err_varargout_not_assigned`.
- **Output before an error.** Each `err_*` case prints one line with `disp`
  first; that line is the ordinary display of the value, not spec text, and
  proves the error comes at run time after the output was flushed.
- **`err_exception_operator_plus`** is the Scope's "same sentence with the
  operator and the class", with the class `MException`, and so is
  `04-switch-try-commands/err_exception_arithmetic.err`, which the Scope
  names.
- **`err_cellfun_recursion_limit`** is cycle 06's
  `Maximum recursion limit of 500 reached.`: `call_nested` passes the error
  out unchanged.
- **`e.stack`.** Nx1, innermost first, no element for the script's frame and
  `file` `''` for a local function (Rules above), so
  `exception_stack_fields` now also asserts one element and an empty
  `file`, and `04-switch-try-commands/err_exception_stack`, which asserted
  the Dot error while `e.stack` did not exist, asserts `struct` and `0`
  elements for an error outside every function. Its name is kept so cycle
  04's Acceptance item and FEATURES row still name it.
- **Deep display.** The Rules record a nested cell as `{1×1 cell}`, never
  expanded, so `cell_deep_nest_display` pins it 100,000 deep.
- **Trailing space and blank line.** `s = ` and the last blank line of a
  display are compared after normalisation, which strips both.
- **Timing.** `struct_chain_freed` nests with `s.a = s` rather than a call to
  `struct` on every link, which halved its run to under three seconds in a
  debug build; item 16's own two forms run in about five, of the harness's
  ten.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/07-cells-and-structs/`. Expected output is written by hand from MATLAB
semantics wherever feasible; the `.err` substring is always chosen by hand.
Every new error message needs an `err_*` case.

1. `c = {1, 'two', [3 4]}; disp(class(c)); disp(c{2}); disp(c{3}(2)); disp(size(c(2:3)))` → `cell\ntwo\n     4\n     1     2`. Cases: cell_literal_brace_and_paren.
2. `c = cell(1, 3); disp(isempty(c{1})); c{5} = 'x'; disp(numel(c)); c(2) = []; disp(numel(c))` → `   1\n     5\n     4` (`isempty` returns a logical, four wide since cycle 02). Cases: cell_grow_and_delete, err_cell_paren_assign_double, err_matrix_paren_assign_cell.
3. `c = {1, 'ab'}` → `c =\n\n  1×2 cell array\n\n    {[1]}    {'ab'}\n`. Cases: cell_display, cell_deep_nest_display.
4. `s.a = 1; s.b = 'hi'; disp(s.a + 1); f = fieldnames(s); disp(f{2}); disp(isfield(s, 'a')); s = rmfield(s, 'a'); disp(isfield(s, 'a'))` → `     2\nb\n   1\n   0` (`isfield` returns a logical). Cases: struct_fieldnames_isfield_rmfield, getfield_setfield, iscell_isstruct, err_missing_field_read, err_fieldnames_not_struct.
5. `s = struct('a', 1)` → `s = \n\n  struct with fields:\n\n    a: 1\n`. Cases: struct_display, err_struct_unpaired, err_struct_cell_sizes.
6. `s = struct('x', 5, 'y', [1 2]); disp(s.y(2)); s.inner.v = 3; s.inner.v = s.inner.v + 1; disp(s.inner.v); n = 'x'; disp(s.(n))` → `     2\n     4\n     5`. Cases: struct_nested_and_dynamic_field, dynamic_field_write, struct_many_fields_by_name, err_invalid_dynamic_field_name, err_dynamic_field_not_char.
7. `p(1).name = 'A'; p(2).name = 'B'; disp(numel(p)); disp(p(2).name); disp(class(p)); q = [p.name]; disp(q)` → `     2\nB\nstruct\nAB`. Cases: struct_array_cs_list, exception_stack_fields, err_struct_assign_dissimilar, err_struct_array_field_assign.
8. `disp(cellfun(@numel, {'ab', 'cde', ''})); r = cellfun(@(x) x * 2, {1, 2}, 'UniformOutput', false); disp(class(r)); disp(r{2})` → `     2     3     0\ncell\n     4`. Cases: cellfun_uniform, cellfun_uniform_output_false, err_cellfun_recursion_limit, err_cellfun_not_cell.
9. `disp(cnt(1, 2, 3)); disp(cnt())\nfunction r = cnt(varargin)\nr = nargin;\nend` → `     3\n     0`; `[a, b] = mv(); disp(b)\nfunction varargout = mv()\nvarargout{1} = 1; varargout{2} = 2;\nend` → `     2`. Cases: varargin_nargin, varargout_two_outputs, err_varargout_not_cell, err_varargout_not_assigned.
10. `[a, b] = deal(7); fprintf('%d %d\n', a, b); c = {1, 2, 3}; disp([c{:}]); disp(cell2mat({1 2; 3 4})); x = num2cell([1 2]); disp(class(x))` → `7 7\n     1     2     3\n     1     2\n     3     4\ncell`. Cases: deal_one_input, cs_list_in_brackets, cell2mat_matrix, num2cell_class, err_cell2mat_nested_cell, err_deal_count.
11. `for c = {1, 'a'}, disp(class(c)), end` → `cell\ncell`. Cases: for_over_cell.
12. `x = 1; x.a = 2` → err `Unable to perform assignment because dot indexing is not supported for variables of this type.`; `c = {1}; c + 1` → err `Operator '+' is not supported for operands of type 'cell'.` (the R2020a wording; the older `Undefined function 'plus' ...` stood here); `c = {1, 2}; y = c{:}` → err `Expected one output from a curly brace or dot indexing expression, but there were 2 results.`. Cases: err_dot_assign_on_double, err_cell_operator_plus, err_brace_cs_list_one_output, err_brace_assign_on_double.
13. `d = [{1}, 2]; disp(class(d)); disp(numel(d))` → `cell\n     2`. Cases: cell_concat_with_number, err_struct_concat_fields.
14. `r = arrayfun(@(x) x * [1 1], 1:2, 'UniformOutput', false); disp(class(r)); disp(r{2})` → `cell\n     2     2`. Cases: arrayfun_uniform_output_false.
15. `f = @sin; disp(size(f)); disp(isempty(f)); try, error('a:b', 'm'), catch e, end; disp(isa(e, 'MException')); disp(numel(e))` → `     1     1\n   0\n   1\n     1`; `f = @sin; f + 1` → err `Operator '+' is not supported for operands of type 'function_handle'.`; `s.a = 1; s * 2` → err `Operator '*' is not supported for operands of type 'struct'.`. Cases: handle_size_isempty, exception_isa_numel, is_predicates_every_value, err_handle_operator_plus, err_struct_operator_times, err_exception_operator_plus.
16. `c = {}; for k = 1:500000, c = {c}; end; clear c; disp(1)` → `     1`, exit 0, and the same through a handle, `c = {}; for k = 1:500000, h = @() c; c = {h}; end; clear c h; disp(2)` → `     2`: freeing a deep chain through the new values never recurses. Cases: cell_chain_freed, cell_handle_chain_freed, struct_chain_freed, chain_alive_at_exit. The two cases through a handle run 250,000 links rather than 500,000, which took about 5 s of the harness's 10 s locally and left too little margin for a slower runner; the crash guard for every chain form is the unit test `deep_chains_through_containers_are_freed_iteratively`, which runs on Rust's 2 MB test thread, where a recursive drop overflows long before either size.
17. Under `--protocol`, after `c = {1, 'a'}; s.x = 1;`, `workspace` lists `c` with class `cell` and size `[1,2]` and `s` with class `struct` and size `[1,1]`. Cases: workspace_lists_cell_struct.
18. `c = {@(x) x + 1, 2}; disp(c{1}(1)); disp(c{2})` → `     2\n     2`. Cases: handle_in_cell_called.

## Status

Done (2026-09-28)
