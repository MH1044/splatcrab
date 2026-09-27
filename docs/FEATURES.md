# Features

What SplatCrab does today, with the golden case that proves each area works.
`Since` is the module that introduced the feature. Cases live under
`tests/cases/`.

## Syntax

| Feature | Since | Golden case | Notes |
|---|---|---|---|
| Numbers `12`, `1.5`, `.5`, `1e-3`, `2.5E+2` | 00 | `display_formats` | |
| Single-quoted strings, `''` escape | 00 | `strings` | Displays with quotes; MATLAB shows char bare |
| Double-quoted strings | 00 | `strings` | Treated as char; MATLAB has a separate string class |
| `%` comments | 00 | every case | |
| `...` line continuation | 00 | `demo_smoke` | |
| `;` suppresses display, `,` and newline show | 00 | `indexing` | |
| Matrix literals, space/comma/newline separators | 00 | `matrix_ops` | `[1 -2]` is two elements, `[1 - 2]` is one |
| Nested concatenation `[A; B]`, `[a' b']` | 00 | `builtins_sample` | |
| Ranges `a:b` and `a:s:b` | 00 | `ranges` | Descending and fractional steps |

## Operators

| Feature | Since | Golden case | Notes |
|---|---|---|---|
| `+ - * /` and left division | 00 | `matrix_ops` | Backslash solves square systems only |
| `^` with an integer exponent | 00 | `demo_smoke` | Negative exponents invert |
| `.* ./ .^` elementwise | 00 | `matrix_ops` | |
| Transpose `'` and `.'` | 00 | `matrix_ops` | |
| `== ~= < <= > >=` | 00 | `logical_ops` | Results are 0/1 doubles until cycle 02 |
| `& \|` elementwise, `&& \|\|` short-circuit | 00 | `logical_ops` | |
| `~` negation | 00 | `logical_ops` | |
| Scalar and row/column broadcasting | 00 | `builtins_sample` | |

## Indexing

| Feature | Since | Golden case | Notes |
|---|---|---|---|
| Linear `A(i)` and two-dimensional `A(i,j)` | 00 | `indexing` | |
| Vector indices `v(2:4)` | 00 | `indexing` | |
| Colon `A(:,1)`, `A(2,:)`, `A(:)` | 00 | `indexing` | |
| `end` anywhere in an index | 00 | `indexing` | Including arithmetic such as `end-1` |
| Indexed assignment with growth | 00 | `growth` | Vector and two-dimensional |
| String indexing | 00 | `strings` | Returns a string |
| Logical indexing | 03 | | Planned |
| Deletion `x(i) = []` | 03 | `err_delete_unsupported` | Currently an error |

## Control flow

| Feature | Since | Golden case | Notes |
|---|---|---|---|
| `if` / `elseif` / `else` | 00 | `control_flow` | |
| `for` over a range | 00 | `control_flow` | |
| `for` over matrix columns | 00 | `control_flow` | |
| `while` | 00 | `control_flow` | |
| `break` and `continue` | 00 | `control_flow` | |

## Builtins

78 names. Every one is exercised by `builtins_sample`, `reductions` or
`demo_smoke`.

| Group | Names | Since |
|---|---|---|
| Constants | `pi e Inf inf NaN nan eps true false` | 00 |
| Constructors | `zeros ones eye rand linspace` | 00 |
| Shape | `size numel length isempty isscalar isvector reshape repmat fliplr flipud` | 00 |
| Reductions | `sum prod mean any all max min cumsum cumprod` | 00 |
| Elementwise math | `abs sqrt exp log log2 log10 sin cos tan asin acos atan sinh cosh tanh floor ceil round fix sign` | 00 |
| Predicates | `isnan isinf isfinite` | 00 |
| Two-argument math | `mod rem atan2 hypot power` | 00 |
| Linear algebra | `transpose inv det trace diag norm dot` | 00 |
| Search and sort | `find sort` | 00 |
| Output | `disp fprintf sprintf num2str error` | 00 |
| Workspace | `clear clc who whos` | 00 |

Reductions take an optional dimension argument. `max` and `min` also take two
arrays. `norm` and `sort` accept vectors only, until cycles 08 and 09.

## Output and formatting

| Feature | Since | Golden case | Notes |
|---|---|---|---|
| Integer, fixed and scientific display | 00 | `display_formats` | Column widths differ from MATLAB above 1000 |
| `Inf`, `-Inf`, `NaN` | 00 | `display_formats` | |
| Empty display | 00 | `display_formats` | Prints `[]`; MATLAB prints a typed header |
| `fprintf` and `sprintf` | 00 | `fprintf_vector` | `%d %i %u %f %e %g %c %s`, flags, width, precision |
| Format cycling over all elements | 00 | `fprintf_vector` | |
| MATLAB-style error messages | 00 | the seven `err_*` cases | |

## Tooling

| Feature | Since | Notes |
|---|---|---|
| REPL with block and bracket continuation | 00 | `exit` and `quit` leave |
| Script runner, exit code 1 on error | 00 | stdout is flushed before the error |
| Golden-file test harness | 00 | `tests/golden.rs`, no dependencies |
| Zero dependencies | 00 | And zero dev-dependencies |
