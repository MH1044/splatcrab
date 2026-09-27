# Features

What SplatCrab does today, with the golden case that proves each area works.
`Since` is the module that introduced the feature. Cases live under
`tests/cases/`.

## Syntax

| Feature | Since | Golden case | Notes |
|---|---|---|---|
| Numbers `12`, `1.5`, `.5`, `1e-3`, `2.5E+2` | 00 | `display_formats` | |
| Single-quoted strings, `''` escape | 00 | `strings` | `s = 'abc'` displays `'abc'` with quotes, as MATLAB R2018a+ does; `disp('abc')` is bare |
| Double-quoted strings | 00 | `strings` | Treated as char; MATLAB has a separate string class |
| `%` comments | 00 | every case | |
| `...` line continuation | 00 | `demo_smoke` | Works straight after a digit, as in `a = 1...` |
| `...` separates elements inside brackets | 01b | `continuation_bracket_element` | `[1 ...` newline `-2]` is two elements, like `[1 -2]` |
| `;` suppresses display, `,` and newline show | 00 | `indexing` | |
| Matrix literals, space/comma/newline separators | 00 | `matrix_ops` | `[1 -2]` is two elements, `[1 - 2]` is one |
| Nested concatenation `[A; B]`, `[a' b']` | 00 | `builtins_sample` | |
| Ranges `a:b` and `a:s:b` | 00 | `ranges` | Descending and fractional steps |
| A range that would not fit is a clean error | 01b | `err_range_too_large` | `1:1e15` used to abort in the allocator; same limit and wording as `check_size` |
| A range lands exactly on its end point | 01d | `range_hits_end_point` | `x = 0:0.1:0.3; x(end) == 0.3` is `1`, and `-1:0.01:1` is symmetric: the upper half is computed from the right-hand end point, not by repeated addition |
| An infinite range end point is refused | 01d | `err_range_end_inf`, `err_range_start_neg_inf` | `0:Inf` and `-Inf:1:0` report `1xInf` rather than quietly giving a 1x0. `1:NaN` is still an empty, and still in Known bugs |

## Operators

| Feature | Since | Golden case | Notes |
|---|---|---|---|
| `+ - * /` and left division | 00 | `matrix_ops` | Backslash solves square systems only |
| The singular test is relative to the matrix | 01d | `solve_relative_pivot` | The pivot tolerance scales with the largest finite magnitude in the matrix, so the perfectly conditioned `[1e-15 0; 0 1e-15] \ [1; 1]` is solved rather than refused. A fixed `1e-14` used to judge it, and `det` and `\` disagreed on what singular means; they now make the identical test |
| `^` with an integer exponent | 00 | `demo_smoke` | Negative exponents invert |
| `.* ./ .^` elementwise | 00 | `matrix_ops` | |
| `.\` elementwise left divide | 01b | `eldiv_vector`, `eldiv_after_number` | `a.\b` is `b./a`; `2.\x` no longer means `2 \ x` |
| Transpose `'` and `.'` | 00 | `matrix_ops` | |
| `== ~= < <= > >=` | 00 | `logical_ops` | Results are 0/1 doubles until cycle 02 |
| `& \|` elementwise, `&& \|\|` short-circuit | 00 | `logical_ops` | |
| `~` negation | 00 | `logical_ops` | |
| Scalar and row/column broadcasting | 00 | `builtins_sample` | |
| A result too big to allocate is a clean error | 01d | `err_zip_result_size`, `err_matmul_result_size`, `err_matmul_size_wraps`, `err_index_result_size` | Broadcasting, `*`, a two-subscript read and a reduction all size their result from their operands, and all judge it before allocating. `ones(1e5,1) + ones(1,1e5)` used to abort the process, exit 134 |
| `*` keeps `Inf` and `NaN` through a zero factor | 01d | `matmul_keeps_inf_and_nan` | `[Inf 0] * [0; 1]` is `NaN`, as in MATLAB. A sparsity shortcut used to skip the multiply and give `0` |
| A result that would be complex is a clean error | 01d | `err_complex_sqrt`, `err_complex_log`, `err_complex_asin`, `err_complex_power_operator`, and five more | `sqrt(-4)`, `log(-1)`, `asin(2)`, `(-8)^(1/3)` and the rest used to return `NaN` and exit 0. Cycle 10 replaces the error with the value |

## Indexing

| Feature | Since | Golden case | Notes |
|---|---|---|---|
| Linear `A(i)` and two-dimensional `A(i,j)` | 00 | `indexing` | |
| Vector indices `v(2:4)` | 00 | `indexing` | |
| Colon `A(:,1)`, `A(2,:)`, `A(:)` | 00 | `indexing` | |
| `end` anywhere in an index | 00 | `indexing` | Including arithmetic such as `end-1` |
| Indexed assignment with growth | 00 | `growth` | Vector and two-dimensional |
| String indexing | 00 | `strings` | Returns a string. A char **variable** only: `'abc'(2)` is a parse error, as indexing any literal is |
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
| A `for` that does not run still assigns its variable | 01d | `for_zero_iterations_assigns_empty` | After `k = 7; for k = []; end`, `k` is the empty; a name that did not exist comes into existence. The exact empty shape MATLAB gives is unsettled, so no case asserts it |

## Builtins

80 names, each an ordinary function in `src/builtins/` registered by name in
`Interp::new`. Every one is exercised by `builtins_sample`, `reductions` or
`demo_smoke`; the shared-arm groups also by the `*_shared_arm` cases in
`01-registry-and-builtins`. Cycle 01 counted 81. Cycle 01c removed `e`, which
MATLAB does not have: `exp(1)` is the MATLAB spelling, and `e` is now an
ordinary name, free to be a variable (`e_is_an_ordinary_name`,
`err_e_undefined`, `err_e_undefined_after_clear`).

| Group | Names | Since | File |
|---|---|---|---|
| Constants | `pi Inf inf NaN nan eps true false` | 00 | `core.rs` |
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
`sum`, `prod`, `mean`, `any` and `all` also take `'all'`, and so do `max` and
`min` as their third argument. `max` and `min` also take two arrays. `norm` and
`sort` accept vectors only, until cycles 08 and 09. `sort` puts `NaN` last
when ascending and first when descending. The argument forms each builtin
takes are in [Builtin arguments](#builtin-arguments).

### Calling convention

| Feature | Since | Golden case | Notes |
|---|---|---|---|
| A builtin that produces no value is legal as a statement | 01 | `disp_statement` | `disp`, `fprintf`, `clc`, `clear`, `who`, bare `tic` and `toc` |
| ... and is an error in an expression | 01 | `err_disp_returns_no_value` | `Too many output arguments.`, replacing `'disp' does not return a value.` |
| Too many input arguments is rejected | 01 | `err_too_many_inputs_*` | Previously extra arguments were ignored |
| A dimension argument must be a positive integer | 01 | `err_reduction_dim_zero`, `err_size_dim_zero` | |
| A negative size is an empty, not an error | 01 | `negative_size_is_empty` | `zeros(-1)` is `0x0` |
| A size that would overflow is a clean error | 01 | `err_huge_size_*` | `zeros(1e10)` used to abort the process |
| `NaN(n)` and `Inf(r,c)` fill a matrix | 01 | `nan_inf_constructors` | `true(n)` and `false(n)` too, since 01c |
| `tic`, `toc` and `toc(t)` | 01 | `tic_toc_value`, `tic_toc_handle` | `t = tic` returns a handle; bare `toc` prints the elapsed time |
| A deeply nested expression does not overflow the stack | 01 | `deep_nesting` | The interpreter runs on a 256 MB thread. This holds below about 96,000 levels; deeper still aborts (Known bugs, cycle 01e) |

### Builtin arguments

Cycle 01 made every builtin reject arguments it did not understand. Cycle 01c
implements the ones MATLAB code actually uses. Cases are in
`01c-builtin-arguments`.

| Feature | Since | Golden case | Notes |
|---|---|---|---|
| `true(n)`, `true(r,c)`, `true(sz)`, and the same for `false` | 01c | `constants_true_false_sizes` | Doubles until cycle 02 gives them the logical class. `pi(2)` stays an error, as in MATLAB (`err_pi_takes_no_size`) |
| `eps(x)`, element-wise, and `eps('double')` | 01c | `eps_spacing`, `err_eps_class_name` | The spacing at `abs(x)`, from the exponent field: `eps(1e308)` is `2^971`, `eps(0)` is `2^-1074`, `eps(Inf)` is `NaN`. `eps('single')` waits for cycle 02 |
| Size vectors: `zeros(size(A))` | 01c | `size_vectors_constructors`, `err_size_vector_column` | `zeros`, `ones`, `eye`, `rand`, `NaN`, `Inf`, `true`, `false`. The vector must be a row |
| `reshape(A, sz)`, `reshape(A, r, [])`, `repmat(A, sz)` | 01c | `size_vectors_reshape_repmat`, `err_reshape_placeholder_divisible`, `err_reshape_two_placeholders` | One `[]` placeholder, for the size that makes the count come out |
| Trailing sizes of `1` | 01c | `trailing_singleton_sizes`, `err_nd_third_size`, `err_nd_zero_third_size`, `err_nd_fourth_size`, `err_nd_size_vector`, `err_nd_reshape` | `zeros(2, 3, 1)` is 2x3. Any other third size, `0` included, is "N-D arrays are not supported."; N-D arrays are not built yet. `eye` still takes two sizes |
| A size past `usize` is named as asked | 01c | `err_size_overflow_named`, `err_size_overflow_g_form`, `err_size_overflow_range_inf` | `zeros(1e300)` reports `1e+300x1e+300`, and `0:1e-300:1e300` reports `1xInf`, not the `usize::MAX` clamp. Indexed growth still names the clamp (cycle 03) |
| `linspace` floors its count | 01c | `linspace_floor_count` | `linspace(0, 1, 2.7)` is two points; a count below 1 is 1x0 |
| `linspace` includes both end points exactly | 01d | `range_hits_end_point` | The last element is the end point itself, not `a + (b-a)*(n-1)/(n-1)` |
| `sort(v, 'descend')`, `sort(v, dim)`, `sort(v, dim, direction)` | 01c | `sort_direction`, `sort_descend_stable`, `err_sort_direction` | Stable in both directions; `NaN` first when descending |
| `find(X, n)`, `find(X, n, 'first')`, `find(X, n, 'last')` | 01c | `find_count`, `err_find_count_zero`, `err_find_count_fraction`, `err_find_direction` | The last `n` stay in ascending order; `n` must be a positive integer |
| `norm(v, p)`: `1`, `2`, any `p > 0`, `Inf`, `-Inf`, `'fro'`, `'inf'` | 01c | `norm_order`, `err_norm_type` | Vectors only until cycle 08. `p = 0` and a negative finite `p` are refused |
| `norm` without overflow; an empty sum is `+0` | 01c | `norm_scaled_and_empty_sum` | `norm([1e200 1e200])` is `1.4142e+200`, not `Inf`. `sum([])`, `norm([])` and `dot([], [])` print `0.0000`, not `-0.0000` |
| `diag(v, k)` and `diag(A, k)` | 01c | `diag_offset`, `err_diag_offset` | A `k` past the matrix gives a 0x1 |
| `num2str(x, n)` and `num2str(x, formatSpec)` | 01c | `num2str_precision`, `err_num2str_precision` | `%.{n}g`, and `sprintf` with the leading spaces trimmed. A non-scalar keeps its one-row output until cycle 11 |
| `round(x, n)`, `round(x, n, 'decimals')`, `round(x, n, 'significant')` | 01c | `round_digits`, `err_round_digits`, `err_round_significant_digits`, `err_round_type` | Any integer `n`, ties away from zero. `round(pi, 20)` is `pi` and `round(5, -400)` is `0` |
| `max` and `min` of an empty follow MATLAB's rule | 01c | `max_min_empty_shape` | `max(zeros(3, 0))` is 1x0, `max(zeros(0, 3))` is 0x3 |
| `dot(A, B)` of matrices, and `dot(A, B, dim)` | 01c | `dot_matrices`, `err_dot_sizes`, `err_dot_vector_length`, `err_dot_dim_orientation` | Column-wise; two vectors of equal length may differ in orientation, but not with `dim` |
| `any` ignores `NaN` | 01c | `any_ignores_nan` | `any(NaN)` is `0`, as the MATLAB page says, and GNU Octave 8.4 agrees. `all(NaN)` is `1` |
| `isvector` of a 1x0 or a 0x1 is true | 01c | `isvector_empty` | A 0x0 is not a vector |
| A bare `toc` needs an earlier bare `tic` | 01c | `toc_after_bare_tic`, `err_toc_before_tic`, `err_toc_value_before_tic`, `err_toc_after_handle_tic` | `t = tic` does not count; `toc(t)` is unaffected |
| `'all'`, and no char is ever a dimension | 01c | `reduction_all_option`, `err_reduction_char_dim`, `err_cumsum_all`, `err_max_char_dim`, `err_size_char` | `sum(A, 'x')` used to reduce along dimension 120 |

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
| `printf` bounds its width and its precision | 01d | `err_printf_precision_f`, `err_printf_precision_e`, `err_printf_width` | At most 8192, for every conversion and both fallbacks. `%.65536f` and `%.65535e` used to panic (exit 101) and `%2147483647d` to build a two-gigabyte pad (exit 134) |
| `%d` prints an integer past 2^63 in full | 01d | `printf_d_past_64_bits` | `fprintf('%d', 1e30)` used to print the `i64` clamp `9223372036854775807` |
| `%.Ns` truncates before it pads | 01d | `printf_s_precision_truncates` | `[%5.2s]` of `'abcdef'` is `[   ab]`, as in C and MATLAB |
| An empty `trace` is `+0` | 01d | `trace_empty_is_positive_zero` | `fprintf('%.4f', trace([]))` printed `-0.0000`; `trace` now goes through `math::sum0` like the other reductions |
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
