# 06 — Function handles

## Goal

`@name`, `@(x) body` with capture at creation, `Value::Func`, calling handle variables, `nargout` propagation through single-call bodies, `feval arrayfun func2str str2func is_function_handle`, lexer rule for `@(…)` inside `[]`/`{}`

After this module a user can do things that were not possible before; every
bullet in Scope must be demonstrable by at least one acceptance test below.

## Scope

- `@name`
- `@(x) body` with capture at creation
- `Value::Func`
- calling handle variables
- `nargout` propagation through single-call bodies
- `feval arrayfun func2str str2func is_function_handle`
- lexer rule for `@(…)` inside `[]`/`{}`

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
`tests/cases/06-function-handles/`. Expected output is written by hand from MATLAB
semantics wherever feasible; the `.err` substring is always chosen by hand.
Every new error message needs an `err_*` case.

1. `f = @(x) x.^2; disp(f(4)); g = @(x, y) x + y; disp(g(1, 2)); z = @() 42; disp(z())` → `    16\n     3\n    42`
2. `a = 10; f = @(x) x + a; a = 0; disp(f(1))` → `    11`
3. `g = @sin; disp(g(0)); h = @sq; disp(h(3))\nfunction r = sq(x)\nr = x * x;\nend` → `     0\n     9`
4. `disp(arrayfun(@(x) x * 2, [1 2 3])); disp(arrayfun(@(a, b) a * b, [1 2], [3 4]))` → `     2     4     6\n     3     8`
5. `disp(feval(@(x) x + 1, 1)); disp(feval('sin', 0))` → `     2\n     0`
6. `disp(func2str(@(x) x.^2 + 1)); f = str2func('@(x) x*3'); disp(f(2)); disp(func2str(@sin))` → `@(x)x.^2+1\n     6\nsin`
7. `f = @(x) x + 1` → `f =\n\n  function_handle with value:\n\n    @(x)x+1\n`; `disp(class(f)); disp(isa(f, 'function_handle'))` → `function_handle\n     1`
8. `f = @(v) max(v); [m, i] = f([1 5 2]); disp(i)` → `     2`
9. `add = @(a) @(b) a + b; add3 = add(3); disp(add3(4))` → `     7`
10. `f = @(x) x; f(1, 2)` → err `Too many input arguments.`
11. `f = @(x) x + 1; f(2)` → `ans =\n\n     3\n`
12. Lexer unit test: `{@(x) x + 1, 2}` tokenizes as two cell elements; `[@(x) x+1]` is a parse error.

## Status

Planned
