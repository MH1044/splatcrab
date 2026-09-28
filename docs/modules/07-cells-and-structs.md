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

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.


## Design notes

Filled in as decisions are made. Start from the
"Key designs to preserve" section of docs/ARCHITECTURE.md and record here:
which files and types changed, which invariants were preserved (column-major
storage, 1-based to 0-based conversion at the index boundary, the `end` stack,
name resolution order, the output sink), and any deviation from MATLAB that
was accepted deliberately.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/07-cells-and-structs/`. Expected output is written by hand from MATLAB
semantics wherever feasible; the `.err` substring is always chosen by hand.
Every new error message needs an `err_*` case.

1. `c = {1, 'two', [3 4]}; disp(class(c)); disp(c{2}); disp(c{3}(2)); disp(size(c(2:3)))` → `cell\ntwo\n     4\n     1     2`
2. `c = cell(1, 3); disp(isempty(c{1})); c{5} = 'x'; disp(numel(c)); c(2) = []; disp(numel(c))` → `     1\n     5\n     4`
3. `c = {1, 'ab'}` → `c =\n\n  1×2 cell array\n\n    {[1]}    {'ab'}\n`
4. `s.a = 1; s.b = 'hi'; disp(s.a + 1); f = fieldnames(s); disp(f{2}); disp(isfield(s, 'a')); s = rmfield(s, 'a'); disp(isfield(s, 'a'))` → `     2\nb\n     1\n     0`
5. `s = struct('a', 1)` → `s = \n\n  struct with fields:\n\n    a: 1\n`
6. `s = struct('x', 5, 'y', [1 2]); disp(s.y(2)); s.inner.v = 3; s.inner.v = s.inner.v + 1; disp(s.inner.v); n = 'x'; disp(s.(n))` → `     2\n     4\n     5`
7. `p(1).name = 'A'; p(2).name = 'B'; disp(numel(p)); disp(p(2).name); disp(class(p)); q = [p.name]; disp(q)` → `     2\nB\nstruct\nAB`
8. `disp(cellfun(@numel, {'ab', 'cde', ''})); r = cellfun(@(x) x * 2, {1, 2}, 'UniformOutput', false); disp(class(r)); disp(r{2})` → `     2     3     0\ncell\n     4`
9. `disp(cnt(1, 2, 3)); disp(cnt())\nfunction r = cnt(varargin)\nr = nargin;\nend` → `     3\n     0`; `[a, b] = mv(); disp(b)\nfunction varargout = mv()\nvarargout{1} = 1; varargout{2} = 2;\nend` → `     2`
10. `[a, b] = deal(7); fprintf('%d %d\n', a, b); c = {1, 2, 3}; disp([c{:}]); disp(cell2mat({1 2; 3 4})); x = num2cell([1 2]); disp(class(x))` → `7 7\n     1     2     3\n     1     2\n     3     4\ncell`
11. `for c = {1, 'a'}, disp(class(c)), end` → `cell\ncell`
12. `x = 1; x.a = 2` → err `Unable to perform assignment because dot indexing is not supported for variables of this type.`; `c = {1}; c + 1` → err `Undefined function 'plus' for input arguments of type 'cell'.`; `c = {1, 2}; y = c{:}` → err `Expected one output from a curly brace or dot indexing expression, but there were 2 results.`
13. `d = [{1}, 2]; disp(class(d)); disp(numel(d))` → `cell\n     2`

## Status

Planned
