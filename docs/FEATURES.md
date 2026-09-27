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
| `...` line continuation | 00 | `demo_smoke` | Works straight after a digit, as in `a = 1...` |
| `...` separates elements inside brackets | 01b | `continuation_bracket_element` | `[1 ...` newline `-2]` is two elements, like `[1 -2]` |
| `;` suppresses display, `,` and newline show | 00 | `indexing` | |
| Matrix literals, space/comma/newline separators | 00 | `matrix_ops` | `[1 -2]` is two elements, `[1 - 2]` is one |
| Nested concatenation `[A; B]`, `[a' b']` | 00 | `builtins_sample` | |
| Ranges `a:b` and `a:s:b` | 00 | `ranges` | Descending and fractional steps |
| A range that would not fit is a clean error | 01b | `err_range_too_large` | `1:1e15` used to abort in the allocator; same limit and wording as `check_size` |

## Operators

| Feature | Since | Golden case | Notes |
|---|---|---|---|
| `+ - * /` and left division | 00 | `matrix_ops` | Backslash solves square systems only |
| `^` with an integer exponent | 00 | `demo_smoke` | Negative exponents invert |
| `.* ./ .^` elementwise | 00 | `matrix_ops` | |
| `.\` elementwise left divide | 01b | `eldiv_vector`, `eldiv_after_number` | `a.\b` is `b./a`; `2.\x` no longer means `2 \ x` |
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

81 names, each an ordinary function in `src/builtins/` registered by name in
`Interp::new`. Every one is exercised by `builtins_sample`, `reductions` or
`demo_smoke`; the shared-arm groups also by the `*_shared_arm` cases in
`01-registry-and-builtins`. The count was 79 before this cycle, not the 78
this table used to claim.

| Group | Names | Since | File |
|---|---|---|---|
| Constants | `pi e Inf inf NaN nan eps true false` | 00 | `core.rs` |
| Constructors | `zeros ones eye rand linspace` | 00 | `core.rs` |
| Shape queries | `size numel length isempty isscalar isvector` | 00 | `core.rs` |
| Rearrangement | `reshape repmat fliplr flipud` | 00 | `linalg.rs` |
| Reductions | `sum prod mean any all max min cumsum cumprod` | 00 | `math.rs` |
| Elementwise math | `abs sqrt exp log log2 log10 sin cos tan asin acos atan sinh cosh tanh floor ceil round fix sign` | 00 | `math.rs` |
| Predicates | `isnan isinf isfinite` | 00 | `math.rs` |
| Two-argument math | `mod rem atan2 hypot power` | 00 | `math.rs` |
| Linear algebra | `transpose inv det trace diag norm dot` | 00 | `linalg.rs` |
| Search and sort | `find sort` | 00 | `linalg.rs` |
| Output | `disp fprintf sprintf num2str error` | 00 | `core.rs` |
| Workspace | `clear clc who whos` | 00 | `core.rs` |
| Timing | `tic toc` | 01 | `core.rs` |

Reductions, and `cumsum` and `cumprod`, take an optional dimension argument;
a dimension past the array's returns the input unchanged and `0` is an error.
`max` and `min` also take two arrays. `norm` and `sort` accept vectors only,
until cycles 08 and 09. `sort` puts `NaN` last.

### Calling convention

| Feature | Since | Golden case | Notes |
|---|---|---|---|
| A builtin that produces no value is legal as a statement | 01 | `disp_statement` | `disp`, `fprintf`, `clc`, `clear`, `who`, bare `tic` and `toc` |
| ... and is an error in an expression | 01 | `err_disp_returns_no_value` | `Too many output arguments.`, replacing `'disp' does not return a value.` |
| Too many input arguments is rejected | 01 | `err_too_many_inputs_*` | Previously extra arguments were ignored |
| A dimension argument must be a positive integer | 01 | `err_reduction_dim_zero`, `err_size_dim_zero` | |
| A negative size is an empty, not an error | 01 | `negative_size_is_empty` | `zeros(-1)` is `0x0` |
| A size that would overflow is a clean error | 01 | `err_huge_size_*` | `zeros(1e10)` used to abort the process |
| `NaN(n)` and `Inf(r,c)` fill a matrix | 01 | `nan_inf_constructors` | `true(n)` and `false(n)` wait for cycle 02 |
| `tic`, `toc` and `toc(t)` | 01 | `tic_toc_value`, `tic_toc_handle` | `t = tic` returns a handle; bare `toc` prints the elapsed time |
| A deeply nested expression does not overflow the stack | 01 | `deep_nesting` | The interpreter runs on a 256 MB thread |

## Output and formatting

| Feature | Since | Golden case | Notes |
|---|---|---|---|
| Integer, fixed and scientific display | 00 | `display_formats` | Column widths differ from MATLAB above 1000 |
| `Inf`, `-Inf`, `NaN` | 00 | `display_formats` | |
| Empty display | 00 | `display_formats` | Prints `[]`; MATLAB prints a typed header |
| `fprintf` and `sprintf` | 00 | `fprintf_vector` | `%d %i %u %f %e %g %c %s`, flags, width, precision |
| Format cycling over all elements | 00 | `fprintf_vector` | |
| The `+` and space flags, and precision on integers | 01 | `printf_plus_space_and_int_precision` | `%+d`, `% d`, `%.3d` |
| `%d` of a non-integer switches to `%e` | 01 | `printf_d_nonintegral` | MATLAB's rule; it used to fall back to `%g` |
| `%s` of a number is its character, and a char argument expands per character | 01 | `printf_string_and_char_args` | `%s` still takes a whole char argument |
| MATLAB-style error messages | 00 | the seven `err_*` cases | Every message text is defined in `src/error.rs` and nowhere else |
| `Error: Line N: <msg>` in script mode | 01b | `err_line_runtime`, `err_line_parse` | Runtime, parse and lex errors alike; stdout is still flushed first |
| An error in a block body names the body's line | 01b | `err_line_in_for_body` | `MError::at` keeps the innermost line |
| The REPL reports errors without a line, and survives them | 01b | `repl_error_has_no_line` | One line per entry, so a number would be noise. The only `.repl` case: it drives the prompt, not a script |

## Tooling

| Feature | Since | Notes |
|---|---|---|
| REPL with block and bracket continuation | 00 | `exit` and `quit` leave |
| Script runner, exit code 1 on error | 00 | stdout is flushed before the error |
| Golden-file test harness | 00 | `tests/golden.rs`, no dependencies |
| Zero dependencies | 00 | And zero dev-dependencies |
