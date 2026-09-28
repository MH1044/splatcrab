# Features

What SplatCrab does today, with the golden case that proves each area works.
`Since` is the module that introduced the feature. Cases live under
`tests/cases/`.

## Syntax

| Feature | Since | Golden case | Notes |
|---|---|---|---|
| Numbers `12`, `1.5`, `.5`, `1e-3`, `2.5E+2` | 00 | `display_formats` | |
| Single-quoted strings, `''` escape | 00 | `strings` | A 1-row `char`, and `''` is a 0x0 char. `s = 'abc'` displays `'abc'` with quotes, as MATLAB R2018a+ does; `disp('abc')` is bare |
| Double-quoted strings | 00 | `strings` | Treated as char; MATLAB has a separate string class |
| A char element is a UTF-16 code unit | 02 | `char_utf16_units` | `length('😀')` is `2` and `double('😀')` is `55357 56832`, as in MATLAB; `disp` and `%s` decode the units back to UTF-8, so the pair prints as one character (QA D37) |
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
| An infinite range *step* follows the documented count | 01e | `range_infinite_step` | `1:Inf:5` is the 1x1 `1`: `fix((k-j)/i)` is `fix(4/Inf)`, which is `0`, and a count of `0` is one element. It used to be a 1x0. A range that runs against its step is still empty, `5:Inf:1` included |
| Chained ranges `1:2:3:4` | 01e | `err_chained_range` | Reads as `(1:2:3):4`, as MATLAB reads it; `parse_range` took at most two colons and did not loop, so it was a parse error. Both spellings then meet the same refusal, since a colon start that is not a scalar is an error here (Known bugs) |
| A leading UTF-8 byte-order mark is skipped | 01e | `bom_is_skipped` | The three bytes `EF BB BF` a Windows editor writes are an encoding marker, not source. A file that is not valid UTF-8 is now decoded leniently rather than refused, so a Windows-1252 comment runs; UTF-16 is still unread (Known bugs) |
| Nesting is bounded, not unbounded | 01e | `err_nesting_parens`, `err_nesting_brackets`, `err_nesting_calls`, `err_nesting_flat_sum` | 10,000 levels of parentheses, brackets, calls, indexes, blocks or chained operators. Past that, a clean error from the parser and the identical one from the evaluator; about 96,000 levels used to abort the process with exit 134 (QA D4) |

## Operators

| Feature | Since | Golden case | Notes |
|---|---|---|---|
| `+ - * /` and left division | 00 | `matrix_ops` | Backslash solves square systems only |
| The singular test is relative to the matrix | 01d | `solve_relative_pivot` | The pivot tolerance scales with the largest finite magnitude in the matrix, so the perfectly conditioned `[1e-15 0; 0 1e-15] \ [1; 1]` is solved rather than refused. A fixed `1e-14` used to judge it, and `det` and `\` disagreed on what singular means; they now make the identical test |
| `^` with an integer exponent | 00 | `demo_smoke` | Negative exponents invert |
| `.* ./ .^` elementwise | 00 | `matrix_ops` | |
| `.\` elementwise left divide | 01b | `eldiv_vector`, `eldiv_after_number` | `a.\b` is `b./a`; `2.\x` no longer means `2 \ x` |
| Transpose `'` and `.'` | 00 | `matrix_ops` | |
| `== ~= < <= > >=` | 00 | `logical_ops` | Results are `logical`, since 02; they were 0/1 doubles |
| `& \|` elementwise, `&& \|\|` short-circuit | 00 | `logical_ops` | `logical` results, since 02 |
| `~` negation | 00 | `logical_ops` | A `logical` result, since 02 |
| The class-propagation table | 02 | `class_propagation`, `concat_class` | Arithmetic, unary minus and unary plus give a double whatever their operands: `true + true` is `2`, `'a' + 1` is `98`, `+'a'` is `97`. Comparisons and the logical operators give a logical. Concatenation gives a char if any operand is one, else a logical only if every operand is: `['a' 66]` is `'aB'`, `[true 2]` a double. A 0x0 `[]` takes no part, so `[[] 'abc']` is a char |
| `&&` and `\|\|` need a logical scalar | 01e | `err_and_non_scalar`, `err_or_empty` | `[1 1] && 1` gave `1` and `[] \|\| 1` gave `1`; MATLAB errors, because the operators need one value to branch on. Short-circuiting is unchanged, so `0 && [1 1]` is still `0` and never looks at the right-hand side |
| A `NaN` cannot become a logical | 01e | `err_if_nan`, `err_and_nan`, `err_not_nan` | `if NaN` was taken as true, `NaN & 1` was `1` and `~NaN` was `0`. MATLAB and Octave both refuse: `NaN's cannot be converted to logicals.` The same conversion serves `if`, `while`, `&`, `\|`, `~`, `&&` and `\|\|` |
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
| String indexing | 00 | `strings` | Returns a char. A char **variable** only: `'abc'(2)` is a parse error, as indexing any literal is |
| `s(:)` of a char is a char column | 01e | `empty_result_shapes` | `size(s(:))` is `3 1`, not `1 3`, since 01e; since 02 the column is a char rather than character codes (QA D17) |
| Indexing keeps the class; indexed assignment keeps the left-hand side's | 02 | `indexed_assign_keeps_class`, `char_arith_and_assign` | `s = 'abc'; s(2) = 'Z'` is `'aZc'`, not `97 90 99`, and growth stays a char too. A logical target stores `logical(value)`, so `x = true(1,3); x(2) = 5` stays logical; a double target stores a char's code, so `y(2) = 'a'` stores `97`. A new variable, or the 0x0 `[]`, takes the class assigned into it |
| A logical index is a clean error | 02 | `err_logical_index` | `x(x > 0)` used to give `5 5 5` in silence, reading the mask as positions (QA D6). Now `Logical indexing is not supported yet.`, reading or assigning, until cycle 03 |
| Logical indexing | 03 | | Planned |
| Deletion `x(i) = []` | 03 | `err_delete_unsupported` | Currently an error |

## Control flow

| Feature | Since | Golden case | Notes |
|---|---|---|---|
| `if` / `elseif` / `else` | 00 | `control_flow` | |
| `for` over a range | 00 | `control_flow` | |
| `for` over matrix columns | 00 | `control_flow` | |
| `for` over a char, and `if` on a char | 02 | `for_over_char`, `if_condition_classes` | `for k = 'abc'` iterates one char at a time, each a `char`; `if 'abc'` is true, as a non-empty array with no zero is |
| `while` | 00 | `control_flow` | |
| `break` and `continue` | 00 | `control_flow` | |
| `break` or `continue` outside a loop is an error | 01e | `err_break_outside_loop`, `err_continue_outside_loop` | It used to unwind out of the whole script, so the statements after it never ran and the process still exited 0 (QA D8). Raised when the statement runs, not when it parses, so the output before it is still printed |
| A `for` that does not run still assigns its variable | 01d | `for_zero_iterations_assigns_empty` | After `k = 7; for k = []; end`, `k` is the empty; a name that did not exist comes into existence. The exact empty shape MATLAB gives is unsettled, so no case asserts it |

## Builtins

88 names, each an ordinary function in `src/builtins/` registered by name in
`Interp::new`. Every one is exercised by `builtins_sample`, `reductions` or
`demo_smoke`, or for the class builtins by the cases in
`02-classes-and-display`; the shared-arm groups also by the `*_shared_arm`
cases in `01-registry-and-builtins`. Cycle 01 counted 81. Cycle 02 added the
eight class builtins. Cycle 01c removed `e`, which
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
| Classes | `class islogical ischar isnumeric isa logical char double` | 02 | `core.rs` |
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
when ascending and first when descending.

Every numeric builtin returns a double, whatever its argument's class:
`abs(true)` and `cumsum('abc')` are doubles. The exceptions, since cycle 02,
are the ones MATLAB makes. The rearrangements `transpose`, `fliplr`, `flipud`,
`repmat`, `reshape` and `sort` keep the class, so `fliplr('abc')` is `'cba'`
(QA D17). `max` and `min` of a logical stay logical. `true` and `false`, and
the predicates `any`, `all`, `isnan`, `isinf`, `isfinite`, `isempty`,
`isscalar`, `isvector`, `islogical`, `ischar`, `isnumeric` and `isa`, return
logicals (`predicates_return_logical`), which is what will let cycle 03's
`x(isnan(x))` select. The argument forms each builtin
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
| A deeply nested expression does not overflow the stack | 01 | `deep_nesting` | The interpreter runs on a 256 MB thread, and since 01e the parser and the evaluator refuse anything past 10,000 levels, so the stack is never reached at all |

### Builtin arguments

Cycle 01 made every builtin reject arguments it did not understand. Cycle 01c
implements the ones MATLAB code actually uses. Cases are in
`01c-builtin-arguments`.

| Feature | Since | Golden case | Notes |
|---|---|---|---|
| `true(n)`, `true(r,c)`, `true(sz)`, and the same for `false` | 01c | `constants_true_false_sizes` | Logical in every size form since cycle 02; doubles before it. `pi(2)` stays an error, as in MATLAB (`err_pi_takes_no_size`) |
| `eps(x)`, element-wise, and `eps('double')` | 01c | `eps_spacing`, `err_eps_class_name` | The spacing at `abs(x)`, from the exponent field: `eps(1e308)` is `2^971`, `eps(0)` is `2^-1074`, `eps(Inf)` is `NaN`. `eps('single')` is refused: there is no single class |
| `class`, `islogical`, `ischar`, `isnumeric`, `isa` | 02 | `class_propagation`, `class_predicates` | `isnumeric` is true of a double only, since MATLAB counts neither logical nor char. `isa(A, 'numeric')` and `isa(A, 'float')` hold `double`, and `isa(A, 'integer')` nothing, since no integer class exists yet |
| `logical`, `char`, `double` | 02 | `conversion_builtins`, `err_logical_nan` | `double('A')` is `65`, `char([72 105])` is `'Hi'`, `logical([2 0 -1])` is `1 0 1`. `logical(NaN)` is `NaN's cannot be converted to logicals.` |
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
| Integer, fixed and scientific display | 00 | `display_formats` | |
| Integer columns widen from 1000 | 02 | `display_integer_widths` | `x = 1000` is `        1000` and `x = [1 1000]` is `           1        1000`: twelve-wide columns from 1000 on, six-wide below it as before. A whole number of `1e9` or more is displayed as a non-integer is |
| The common scale factor | 02 | `display_scale_factor` | A matrix whose largest magnitude is outside `[0.01, 1000)` is printed as `A / 10^k` under one `   1.0e+03 *` line, once above any column blocks: `[1.5 1000.5]` is `0.0015 1.0005` under `1.0e+03 *` |
| An exact zero in a fixed-point row is `0` | 02 | `display_scale_factor` | `[0 1.5]` is `         0    1.5000`; it used to print `0.0000` |
| A scalar outside the fixed-point range is `e` format | 02 | `display_scalar_range` | `x = 1234.5` is `   1.2345e+03`, `x = 0.001` is `   1.0000e-03` and `x = 1e10` is `   1.0000e+10` (QA D20) |
| Logical display | 02 | `logical_scalar_display`, `logical_row_display` | `x = 5 > 3` shows `  logical` and `   1`; an array shows `  1×3 logical array`. Logical columns are four wide, so `disp(3 > 1)` is `   1` (QA D38) |
| Char display | 02 | `char_display` | A 1-row char is quoted, `    'abc'`; a multi-row char has a `  2×2 char array` header and one quoted row per line. `disp` is bare, one line per row |
| `Inf`, `-Inf`, `NaN` | 00 | `display_formats` | |
| A non-finite element keeps the integer columns | 01e | `disp_nonfinite_keeps_integers` | `disp([1 2 NaN])` is `     1     2   NaN`; it used to force the whole row to four decimals. A non-finite value has no digits, so it neither changes the format nor widens the column: `[NaN Inf -Inf 1]` is four six-wide columns |
| A wide matrix wraps into column blocks | 01e | `wide_matrix_wraps` | 80 characters, whether the output is a terminal or a pipe, giving 8 fixed-point columns or 13 integer ones per block under a `Columns N through M` heading. `linspace(1, 2)` printed about 1300 characters on one line |
| Empty display | 00 | `display_formats` | `x = []` still prints `     []`, as MATLAB does for a 0x0 double |
| Typed empty headers | 02 | `typed_empty_display`, `empty_char_display` | `zeros(0,3)` shows `  0×3 empty double matrix`, `1:0` `  1×0 empty double row vector`, `zeros(0,1)` `  0×1 empty double column vector` and `''` `  0×0 empty char array`; a logical empty follows the pattern with `logical array` |
| `who` and `whos` show the class | 02 | | A logical is `logical` and a char has its real shape, `2x2 char`; they used to be `double` and `1xN char` |
| `disp([])` prints nothing | 01e | `empty_result_shapes` | It used to print `     []`. `disp('')` is still a line with nothing on it |
| Empty results have MATLAB's shapes | 01e | `empty_result_shapes` | `find([])` and `diag([])` are `0x0`, not `0x1`; `size('')` is `0 0`, not `1 0`, and `num2str([])` follows it. A shape with an orientation to keep still keeps it: `find([0 0])` is `1x0` |
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
| An error in an `elseif` condition names the `elseif` | 01e | `err_elseif_line` | It reported the `if`'s line, since the whole statement carried one line (QA D27). Each arm now carries its condition's own line |
| A parse error names the token as it is written | 01e | `err_parse_token_semi`, `err_parse_token_ident`, `err_parse_token_number` | `y = x + ;` reports `unexpected ';' in expression`, not `unexpected Semi`. `Token` has a `Display` form that every parse message uses: a quoted spelling for everything with one, and `end of line` / `end of input` for the two without |
| The message text follows MATLAB R2020a | 01e | `err_undefined_wording` | `Unrecognized function or variable 'x'.`, the wording of R2020a and later; it used to be `Undefined ...`. See the policy in `docs/modules/01e-display-and-parser.md`, including the two messages deliberately kept because they say more than MATLAB's |
| The REPL reports errors without a line, and survives them | 01b | `repl_error_has_no_line` | One line per entry, so a number would be noise |
| REPL diagnostics go to stderr | 01e | `repl_error_to_stderr` | Script mode already did, so a piped session can now separate diagnostics from output too |
| An unterminated block at end of input is reported | 01e | `err_repl_unterminated_block` | Piping `for k = 1:3` and `disp(k)` with no `end` printed nothing and exited 0 (QA D36); it now exits 1 and says why |

## Tooling

| Feature | Since | Notes |
|---|---|---|
| REPL with block and bracket continuation | 00 | `exit` and `quit` leave |
| Script runner, exit code 1 on error | 00 | stdout is flushed before the error |
| The Windows console reads output as UTF-8 | 02 | `SetConsoleOutputCP(65001)`, a raw `extern "system"` declaration in `src/main.rs` rather than a crate, so `×` and non-ASCII text render. A pipe or a file is unaffected |
| Golden-file test harness | 00 | `tests/golden.rs`, no dependencies |
| Zero dependencies | 00 | And zero dev-dependencies |
