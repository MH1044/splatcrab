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
| Block comments `%{ ... %}` | 04 | `block_comment`, `block_comment_skips_code`, `block_comment_deep` | `%{` and `%}` each alone on its line, surrounding whitespace allowed; they nest, counted rather than recursed into. A marker with anything else on its line is an ordinary comment. An unterminated `%{` runs to the end of a script as a comment, and keeps the REPL reading. The lines between them used to execute (QA D7) |
| Command syntax | 04 | `command_disp_word`, `command_clear_two_words`, `command_variable_expression`, `err_command_clear_one`, `err_command_clear_all`, `err_command_hold_unrecognized`, `err_command_format_unrecognized` | MATLAB's rule: a statement that starts with a name that is not a variable, then whitespace, then a word that is not an operator followed by whitespace, calls the name with each word as a char argument. Quotes group words; the command ends at a newline, `,`, `;` or `%` outside quotes. `clear x y`, `clear all` and `disp hello` work; `x -1` with `x` a variable stays `x - 1`. Whether a name is a variable is decided before the source runs, from the workspace and the names it has assigned so far. `hold on` and `format long` are the unrecognized-name error until cycles 12 and 13. It used to be a parse error (QA D31) |
| `...` line continuation | 00 | `demo_smoke` | Works straight after a digit, as in `a = 1...` |
| `...` separates elements inside brackets | 01b | `continuation_bracket_element` | `[1 ...` newline `-2]` is two elements, like `[1 -2]` |
| `;` suppresses display, `,` and newline show | 00 | `indexing` | |
| Matrix literals, space/comma/newline separators | 00 | `matrix_ops` | `[1 -2]` is two elements, `[1 - 2]` is one |
| Nested concatenation `[A; B]`, `[a' b']` | 00 | `builtins_sample` | |
| Ranges `a:b` and `a:s:b` | 00 | `ranges` | Descending and fractional steps |
| A range that would not fit is a clean error | 01b | `err_range_too_large` | `1:1e15` used to abort in the allocator; same limit and wording as `check_shape` |
| A range lands exactly on its end point | 01d | `range_hits_end_point` | `x = 0:0.1:0.3; x(end) == 0.3` is `1`, and `-1:0.01:1` is symmetric: the upper half is computed from the right-hand end point, not by repeated addition |
| An infinite range end point is refused | 01d | `err_range_end_inf`, `err_range_start_neg_inf` | `0:Inf` and `-Inf:1:0` report `1xInf` rather than quietly giving a 1x0. `1:NaN` is still an empty, and still in Known bugs |
| An infinite range *step* follows the documented count | 01e | `range_infinite_step` | `1:Inf:5` is the 1x1 `1`: `fix((k-j)/i)` is `fix(4/Inf)`, which is `0`, and a count of `0` is one element. It used to be a 1x0. A range that runs against its step is still empty, `5:Inf:1` included |
| Chained ranges `1:2:3:4` | 01e | `err_chained_range` | Reads as `(1:2:3):4`, as MATLAB reads it; `parse_range` took at most two colons and did not loop, so it was a parse error. Both spellings then meet the same refusal, since a colon start that is not a scalar is an error here (Known bugs) |
| A leading UTF-8 byte-order mark is skipped | 01e | `bom_is_skipped` | The three bytes `EF BB BF` a Windows editor writes are an encoding marker, not source. A file that is not valid UTF-8 is now decoded leniently rather than refused, so a Windows-1252 comment runs; UTF-16 is still unread (Known bugs) |
| The tokens `{ }`, the field `.` and `@` | 03 | `err_brace_on_matrix`, `err_dot_on_matrix` | `c{1}`, `s.a` and `s.(n)` lex and parse, alone or chained (`c{1}(2).b`), without disturbing `1.5`, `.5`, `x.^2`, `x.*y`, `x./y`, `x.\y`, `x.'` or a `...` continuation. Inside brackets a brace or an `@` after a space starts an element. A bare `@` is `unexpected '@' in expression` until cycle 06's function handles, and a `{` opening a value is a parse error until cycle 07's cells |
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
| Logical indexing, reading and assigning | 03 | `logical_mask_read`, `logical_mask_assign`, `logical_mask_matrix_column`, `logical_mask_all_true` | A logical subscript is a mask, never a list of positions: `x(x > 2)`, `x(x > 4) = 0`, `A(A > 5)`. A mask with no zeros is still a mask, so `x(x > 0)` of `[5 6 7]` is `5 6 7`; until cycle 02 it was `5 5 5` in silence (QA D6), and in cycle 02 a clean error. A double of ones and zeros is still positions, as in MATLAB |
| A mask indexes as `find(mask)` would | 03 | `mask_find_shape`, `mask_shorter_than_array`, `mask_assign_grows`, `err_mask_true_past_end` | The mask becomes the positions `find` returns, in `find`'s shape, and then indexes as a numeric index would: a mask on a vector keeps the vector's orientation, a row mask on a matrix gives a row, a matrix mask a column. A shorter mask selects among the elements it covers; a `true` past the end is the out-of-bounds error on read and grows the array on assignment. Masks work in either subscript of two, `A(mask, :)` |
| Deletion `x(i) = []` | 03 | `delete_vector_elements`, `delete_linear_from_matrix`, `delete_column_and_row`, `err_delete_two_indices` | By position or by mask. A vector keeps its orientation, a linear deletion from a matrix leaves a row, `x(:) = []` a 0x0; `A(:, j) = []` and `A(i, :) = []` remove columns and rows, and a subscript spanning its whole dimension (`1:end`) counts as a colon. Two subscripts that each select part of their dimension are `A null assignment can have only one non-colon index.` Only the literal `[]` deletes: `e = []; x(2) = e` is an assignment, and a count mismatch. The class is kept |
| Indexed assignment in place | 03 | `growth_perf_guard`, `repl_failed_assign_unchanged` | Every subscript, the class conversion, the growth and the element count are checked before the variable changes, so a failed assignment leaves it exactly as it was. A multiple assignment `[a, b] = ...` assigns its targets one at a time, so one that fails part-way keeps the targets already assigned. The variable is then changed where it is stored, never copied, and growth along its last dimension is an amortised resize, so `z(end+1) = k` 200000 times is linear. A read `x(k)` no longer copies `x` either |
| Growth names the size asked for | 03 | `err_growth_size_named` | `x = []; x(1e300) = 1` reports `Requested 1x1e+300 array exceeds the maximum array size.`, not the `1x18446744073709551615` of the saturated `usize`: the grown size stays an `f64` until `check_shape` judges it |
| Trailing singleton subscripts | 03 | `trailing_singleton_subscripts`, `trailing_singleton_end`, `err_trailing_singleton_bound` | `A(2, 1, 1)`, `A(:, :, 1)` and `A(1, 2, 1) = 9` index a 2-D matrix, `end` is `1` in a third or later position, and a third subscript past 1 is `Index in position 3 exceeds array bounds. Index must not exceed 1.` (QA D22). Selecting a second page, or growing into one, is `N-D arrays are not supported.` |
| The invalid-index message names logical values | 03 | `err_index_zero_ending` | `x(0)` is `Index in position 1 is invalid. Array indices must be positive integers or logical values.`, MATLAB's text, now that logical indices exist |
| Brace and dot access on a matrix are errors | 03 | `err_brace_on_matrix`, `err_dot_on_matrix` | `x{1}` is `Brace indexing is not supported for variables of this type.` and `x.a` is `Dot indexing is not supported for variables of this type.`, reading or assigning; both stay right for a matrix once cycle 07 adds cells and structs. A second `(...)` indexes the value so far, `x(2:3)(2)` |

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
| `switch` / `case` / `otherwise` | 04 | `switch_otherwise`, `switch_char_case`, `switch_cell_case`, `switch_break_in_for`, `err_switch_not_scalar`, `err_nesting_switch`, `err_switch_stray_statement`, `err_case_without_switch` | The first matching `case` runs, with no fall-through. A number matches a number of equal value whatever its class; a char matches a char of the same text and never a number by its code. `case {a, b}` matches any of its values, and is syntax rather than a cell until cycle 07. `break` and `continue` inside act on the enclosing loop. A subject that is neither a scalar nor a character vector is `SWITCH expression must be a scalar or a character vector.` |
| `try` / `catch` | 04 | `try_catch_message`, `try_catch_identifier`, `try_catch_undefined`, `try_rethrow_nested`, `rethrow_keeps_identifier`, `err_rethrow_uncaught`, `exception_class`, `err_exception_stack`, `err_exception_arithmetic`, `err_rethrow_not_exception`, `err_nesting_try` | Every runtime error inside `try` is caught, a builtin's included. `catch e` on the same line binds a minimal `MException`: `e.message`, `e.identifier`, `class(e)` is `'MException'`, and any other field is the Dot error until cycle 07's `e.stack`, a struct array. `catch` followed by a comma or a newline binds nothing, and a `try` with no `catch` ignores the error. `break` and `continue` pass through. `rethrow(e)` raises it again unchanged, line included |
| A `for` that does not run still assigns its variable | 01d | `for_zero_iterations_assigns_empty` | After `k = 7; for k = []; end`, `k` is the empty; a name that did not exist comes into existence. The exact empty shape MATLAB gives is unsettled, so no case asserts it |

## Functions

Cases in `05-functions-and-scoping/`.

| Feature | Since | Golden case | Notes |
|---|---|---|---|
| Local functions at the end of a script | 05 | `local_function_call`, `statement_call_ans`, `err_function_before_statement` | Every header form: `function name`, `function name(a, b)`, `function y = name(...)`, `function [y, z] = name(...)`, `~` for an ignored argument. A statement after a function is `Function definitions in a script must appear at the end of the file.` A call as a statement that returns a value sets and shows `ans` |
| Multiple outputs | 05 | `multiple_outputs`, `err_output_not_assigned` | `[s, p] = f(...)` through cycle 03's multiple assignment. An output asked for and never assigned is `Output argument "y" (and maybe others) not assigned during call to "f".`; asking for more outputs than the function has is `Too many output arguments.` |
| `nargin` and `nargout` | 05 | `nargin_default`, `nargout_values`, `err_nargin_outside_function` | `nargout` is `1` in an expression, `0` as a statement and the target count of `[a, b] = f()`. Arguments may be left off the end; one too many is `Too many input arguments.`, raised before the call, with no trace line (`err_too_many_inputs`). Outside every function both are `You can only call nargin/nargout from within a MATLAB function.` |
| `return` | 05 | `early_return` | Leaves the function from inside any loop or block; at the top of a script it ends the script |
| A workspace per call | 05 | `err_caller_variable_invisible`, `end_inside_call` | A function sees only its arguments and what it assigns; `end` is resolved in the running frame, so `x(f(end))` binds `end` to `x` and a function's own indexing never sees the caller's (invariant 3). `break` in a function never reaches a loop in its caller |
| Recursion, limited to 500 calls | 05 | `fact_recursion`, `recursion_deep_frames`, `err_recursion_limit`, `err_recursion_nested_expression` | The 501st nested call is `Maximum recursion limit of 500 reached.`, a clean error. Every frame shares the one nesting budget of 10,000 levels, so 500 frames deep in expressions end in one clean error or the other, never an abort (invariant 6) |
| The error trace | 05 | `err_trace_two_frames`, `err_eval_error_line_outermost` | An uncaught error that left a function prints `Error: Line N: <msg>`, `N` the script's own line, then one `  in <fn> (line N)` per function it left, innermost first, to stderr. The REPL prints the message and the trace without a line |
| Function files and subfunctions on a path | 05 | `path_test`, `err_subfunction_private`, `subfunction_before_script_function`, `local_shadows_path_file` | `name.m` in the current folder or on the path, starting with `function`: its first function is what `name` calls, the rest are private to the file. A file's functions may all end with `end` or all go without |
| Scripts on the path run in the caller's workspace | 05 | `script_on_path`, `script_in_function_workspace` | Called from a function, a script fills the function's workspace. A script takes no arguments and gives no outputs |
| Name resolution, and user files shadow builtins | 05 | `variable_shadows_function`, `local_shadows_path_file`, `addpath_shadow` | Variable, then the running file's local functions, then the script's, then the current folder, then the path, then a builtin (invariant 4) |
| `addpath` and `rmpath` | 05 | `addpath_shadow`, `addpath_after_lookup`, `err_path_folder_warnings` | `addpath` puts folders at the front of the path, `rmpath` takes them off, both relative to `Interp::cwd`. A folder that does not exist, or is not on the path, is a warning. Each bumps the file cache's generation, so a lookup cached before is never used after |
| `exist` and `feval` | 05 | `exist_kinds`, `exist_path_file`, `feval_local_function` | `exist` is `1` for a variable, `2` for a file on the path, `5` for a builtin, `0` otherwise. `feval('name', ...)` calls a function by name, variables excepted |
| Definitions refused at the REPL, the protocol and the page | 05 | `err_repl_function_refused`, `err_repl_function_body_not_run`, `err_eval_function_refused`, `complete_function_block`, `err_eval_error_line_outermost` | `Function definitions are not supported in this context.` `syntax::is_complete` counts `function ... end` as a block, so the whole definition is read first and none of it runs. A protocol error inside a function keeps `line` the submitted code's own |

## Builtins

99 names, each an ordinary function in `src/builtins/` registered by name in
`Interp::new`. Every one is exercised by `builtins_sample`, `reductions` or
`demo_smoke`, or for the class builtins by the cases in
`02-classes-and-display`; the shared-arm groups also by the `*_shared_arm`
cases in `01-registry-and-builtins`. Cycle 01 counted 81. Cycle 02 added the
eight class builtins, and cycle 04 five: `rethrow`, `lasterr`, `warning`,
`assert` and `isequal`, each exercised by the `04-switch-try-commands` cases.
Cycle 05 added six, `nargin`, `nargout`, `exist`, `feval`, `addpath` and
`rmpath`, exercised by the `05-functions-and-scoping` cases. Cycle 01c removed `e`, which
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
| Output | `disp fprintf sprintf num2str` | 00 | `core.rs` |
| Errors and warnings | `error rethrow lasterr warning assert` | 00, 04 | `core.rs` |
| Comparison | `isequal` | 04 | `core.rs` |
| Workspace | `clear clc who whos` | 00 | `core.rs`; `clear all` since 04 |
| Timing | `tic toc` | 01 | `core.rs` |
| Functions and the path | `nargin nargout exist feval addpath rmpath` | 05 | `core.rs` |

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
`x(isnan(x))` select, and since cycle 03 it does. The argument forms each builtin
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
| Multiple assignment `[a, b] = f(...)` | 03 | `multi_assign_max_display`, `multi_assign_size`, `multi_assign_sort`, `multi_assign_find`, `multi_assign_tilde_min` | The call is asked for as many values as there are targets, which are assigned and then shown in order unless the statement ends in `;`. `~` takes an output and discards it; a target may be indexed, `[v(2), k] = max(w)`; `[x] = f(...)` is the one-target form. A multiple assignment does not set `ans`. It used to be a parse error (QA D32) |
| ... and its two errors | 03 | `err_multi_assign_insufficient`, `err_multi_assign_too_many` | `[a, b] = 5`, or any value that is not a call, is `Insufficient number of outputs from right hand side of equal sign to satisfy assignment.`; a builtin asked for more values than it has, `[a, b] = sum(x)`, is `Too many output arguments.` |
| `[m, i] = max(...)`, `[m, i] = min(...)` | 03 | `multi_assign_max_display`, `multi_assign_tilde_min` | The index of each extremum along the dimension reduced, the first of a tie, ignoring `NaN`; with `'all'` a linear index. The two-array form has no index |
| `[s, i] = sort(...)` | 03 | `multi_assign_sort` | The permutation, so `s` is `v(i)`; stable in both directions |
| `[r, c] = size(A)` | 03 | `multi_assign_size` | One dimension per output, and outputs past the second are `1`, the trailing singletons; `[n] = size(A)` is still the size row |
| `[r, c] = find(X)`, `[r, c, v] = find(X)` | 03 | `multi_assign_find` | Row and column subscripts, and the values in the argument's class, in `find`'s shape; the count and direction still apply |
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
| A size past `usize` is named as asked | 01c | `err_size_overflow_named`, `err_size_overflow_g_form`, `err_size_overflow_range_inf` | `zeros(1e300)` reports `1e+300x1e+300`, and `0:1e-300:1e300` reports `1xInf`, not the `usize::MAX` clamp. Indexed growth does too since cycle 03 (`err_growth_size_named`) |
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
| `error`'s argument rules | 04 | `err_error_percent_literal`, `err_error_escape_literal`, `error_empty_no_throw`, `err_error_format`, `err_error_identifier` | From the MATLAB `error` page (QA D9). One argument is literal, with no format or escape processing: `error('100% sure')` says `100% sure`, and used to say `100ure`. When every input is empty nothing is thrown. With more arguments, a first argument with a colon and no whitespace is the identifier and the rest the format and its values: `error('MyPkg:myid', 'Value %d bad', 7)` says `Value 7 bad` |
| `warning` | 04 | `warning_to_stderr`, `warning_in_protocol_out` | `Warning: <msg>` on the second sink, `Interp.err`, by `error`'s argument rules; the script goes on and exits 0. It is stderr in a script and at the REPL, with stdout flushed first so the two stay in order on one terminal; under `--protocol` and `--ui` it is the same capture as `out`, so nothing reaches stderr there |
| `assert` | 04 | `err_assert_message`, `err_assert_no_message` | `assert(cond)` is `Assertion failed.` when `cond` fails the test `if` uses; `assert(cond, fmt, ...)` raises that message, read by `error`'s rules |
| `isequal` | 04 | `isequal_logical` | Two or more arguments; true when every one has the first's size and values. The class is not compared, so `isequal('a', 97)` is true; a `NaN` equals nothing |
| `lasterr` | 04 | `lasterr_message` | The message of the last error, caught or not; `''` before the first |
| MATLAB-style error messages | 00 | the seven `err_*` cases | Every message text is defined in `src/error.rs` and nowhere else |
| `Error: Line N: <msg>` in script mode | 01b | `err_line_runtime`, `err_line_parse` | Runtime, parse and lex errors alike; stdout is still flushed first |
| An error in a block body names the body's line | 01b | `err_line_in_for_body` | `MError::at` keeps the innermost line |
| An error in an `elseif` condition names the `elseif` | 01e | `err_elseif_line` | It reported the `if`'s line, since the whole statement carried one line (QA D27). Each arm now carries its condition's own line |
| A parse error names the token as it is written | 01e | `err_parse_token_semi`, `err_parse_token_ident`, `err_parse_token_number` | `y = x + ;` reports `unexpected ';' in expression`, not `unexpected Semi`. `Token` has a `Display` form that every parse message uses: a quoted spelling for everything with one, and `end of line` / `end of input` for the two without |
| The message text follows MATLAB R2020a | 01e | `err_undefined_wording` | `Unrecognized function or variable 'x'.`, the wording of R2020a and later; it used to be `Undefined ...`. See the policy in `docs/modules/01e-display-and-parser.md`, including the two messages deliberately kept because they say more than MATLAB's |
| The REPL reports errors without a line, and survives them | 01b | `repl_error_has_no_line` | One line per entry, so a number would be noise |
| REPL diagnostics go to stderr | 01e | `repl_error_to_stderr` | Script mode already did, so a piped session can now separate diagnostics from output too |
| An unterminated block at end of input is reported | 01e | `err_repl_unterminated_block`, `err_repl_unterminated_switch` | Piping `for k = 1:3` and `disp(k)` with no `end` printed nothing and exited 0 (QA D36); it now exits 1 and says why |

## Evaluation protocol

`splatcrab --protocol`, the groundwork for the interface (cycle U0). Cases in
`tests/cases/U0-ui-foundations/`, each a `.proto` session.

| Feature | Since | Golden case | Notes |
|---|---|---|---|
| `splatcrab --protocol`, JSON Lines over one session | U0 | `eval_display_and_session`, `err_eval_answer_session_survives`, `handbook_protocol_example` | One JSON request per line on stdin, one JSON response per line on stdout, flushed after each. Blank lines and a trailing `\r` are skipped. Exits 0 at end of input whatever the requests did, and writes nothing to stderr |
| Request ids | U0 | `string_id_echoed`, `workspace_sorted_with_class` | A number or a string `id` is echoed as the response's first key; none, or `null`, is answered `"id":null` |
| `eval` | U0 | `eval_display_and_session`, `err_eval_answer_session_survives`, `err_eval_keeps_earlier_assignments`, `err_eval_unclosed_block` | Runs `code` as a REPL entry, output captured into `out`. An error adds `error: {message, line}`: the message as the REPL prints it, the line one-based within `code`. Variables assigned before an error survive it. Code with an open block is run as sent and fails, rather than waiting for more |
| `complete` | U0 | `complete_open_and_closed`, `complete_switch_try_comment`, `complete_try_open_switch_closed` | Whether `code` is a finished entry: `syntax::is_complete`, the same function the REPL asks. Since cycle 04 it counts `switch` and `try` as block openers and an open `%{` as unfinished |
| A warning is part of `out` | 04 | `warning_in_protocol_out` | `eval` points both of the interpreter's sinks at one capture, so a warning sits in `out` in the order it was raised and nothing reaches stderr |
| `workspace` | U0 | `workspace_sorted_with_class` | `{name, size, class}` per variable, sorted by name |
| `completions` | U0 | `completions_builtins_and_variables`, `completions_shadow_listed_once` | Every variable and builtin starting with `prefix`, sorted by byte order, a shadowed builtin listed once: `env::completions` |
| Malformed requests are answers | U0 | `err_malformed_not_json`, `err_malformed_not_object`, `err_malformed_no_op`, `err_malformed_eval_no_code`, `err_malformed_field_not_string`, `err_malformed_bad_id`, `err_unknown_operation` | `"ok":false` with `Malformed request: <what>.` or `Unknown operation '<op>'.` and `"line":null`, then the next line is read |
| JSON escaping | U0 | `escape_quote_and_backslash`, `escape_tab`, `raw_utf8_times` | `\"`, `\\`, `\n`, `\r`, `\t`, `\u00xx` for any other control character, raw UTF-8 for everything else, `×` included |
| Nesting is bounded | U0 | `err_deep_nesting` | `src/json.rs` refuses arrays or objects nested past 128 levels, so 100,000 `[` are a malformed request, not a stack overflow |

## UI server

`splatcrab --ui`, the command window in the browser (cycle U1). Cases in
`tests/cases/U1-ui-server/`, each an `.http` session run with port 8123 and
token `test-token`; the real socket is `tests/ui_server.rs`, and every
status, header and limit is also a unit test in `src/http.rs`.

| Feature | Since | Golden case | Notes |
|---|---|---|---|
| `splatcrab --ui` | U1 | `tests/ui_server.rs` | Binds `127.0.0.1` only, on `--port N` or a port the system picks; prints `SplatCrab UI: http://127.0.0.1:<port>/#<token>`, flushes it, and opens it with `cmd /c start`, `open` or `xdg-open` unless `--no-browser`, ignoring a failure. One connection at a time on the interpreter thread; a request not received within 10 seconds of the connection, or a response not taken within 10 seconds, closes the connection, however slowly the bytes trickle, and no client can end the server |
| The session token | U1 | `err_token_missing_never_evaluated`, `err_token_wrong_never_evaluated` | 128 bits, 32 lower-case hex digits, from `RandomState` mixed with the time and the process id; `--token T` fixes it. It rides in the URL fragment and comes back in `X-SplatCrab-Token`, compared in constant time; without it `/api` is `403` |
| `Host` must name this server | U1 | `err_host_foreign_never_evaluated`, `err_host_wrong_port_never_evaluated`, `host_localhost_accepted`, `err_host_foreign_static_route` | `127.0.0.1:<port>` or `localhost:<port>` on every request, static routes included, against DNS rebinding; anything else, or none, is `403` |
| `Origin`, when sent, must be this server | U1 | `err_origin_foreign_never_evaluated`, `err_origin_wrong_port_never_evaluated`, `err_origin_null_never_evaluated`, `origin_loopback_accepted` | `http://127.0.0.1:<port>` or `http://localhost:<port>`; `null` and every other origin are `403` |
| A refused request never reaches the interpreter | U1 | the `*_never_evaluated` cases | Each follows a refusal with a valid request showing the refused code never ran |
| `POST /api` | U1 | `eval_round_trip`, `session_persists_across_requests`, `malformed_protocol_body_answered` | The body is one U0 request and the answer is `protocol::respond`'s JSON less its newline, as `application/json`; the session persists across requests, and a malformed body is a `200` carrying the protocol's own refusal |
| Routes and methods | U1 | `err_not_found`, `err_api_wrong_method`, `err_page_wrong_method`, `err_api_wrong_content_type_never_evaluated` | `GET /`, `/app.js`, `/app.css` and `POST /api`; anything else `404`, the wrong method `405` with `Allow`, `/api` without `application/json` `415` |
| Malformed HTTP | U1 | `err_request_line_not_http`, `err_header_line_no_colon`, `err_content_length_not_a_number`, `err_transfer_encoding_chunked` | `400` for a bad request line, header or `Content-Length`; `501` for any `Transfer-Encoding` |
| Size limits before buffering | U1 | `err_body_too_large`, `err_headers_too_large`, `err_header_line_too_large`, `err_request_line_too_large` | A head past 16 KiB is `431` after reading one byte more than the cap; a `Content-Length` past 8 MiB is `413` without reading the body |
| Header names in any case, CRLF or LF | U1 | `header_names_any_case` | Values are trimmed of spaces and tabs |
| Deterministic responses | U1 | every `U1-ui-server` case | Status line, `Content-Type`, `Content-Length`, `Cache-Control: no-store`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`, the CSP on `GET /` only, `Allow` on a `405` only, `Connection: close`; no `Date`, no `Server`. An error's body is its status text, such as `403 Forbidden` |
| `splatcrab --http-stdio --port N --token T` | U1 | every `U1-ui-server` case | Requests from stdin one after another, empty lines before a request line skipped, each response followed by one `\n`; exits 0 at end of input and writes nothing to stderr |
| The command window | U1 | checked by hand; `src/http.rs` unit tests | A transcript of entries, each its input then its exact output in a monospaced block, an error set apart; Enter runs an entry when `complete` says it is finished and inserts a newline otherwise; Up and Down walk the history; light and dark follow the system. No inline script or style, under `Content-Security-Policy: default-src 'self'; frame-ancestors 'none'` |

## Tooling

| Feature | Since | Notes |
|---|---|---|
| REPL with block and bracket continuation | 00 | `exit` and `quit` leave. Since U0 the continuation test is `syntax::is_complete`, shared with the protocol's `complete` |
| `.proto` golden cases | U0 | `tests/golden.rs` spawns the binary with `--protocol` and types the case on stdin |
| `.http` golden cases | U1 | `tests/golden.rs` spawns the binary with `--http-stdio --port 8123 --token test-token` and pipes the case on stdin; an `.http` case with no `.err` asserts an empty stderr |
| The UI server over a real socket | U1 | `tests/ui_server.rs` spawns `--ui --port 0 --no-browser --token itest` and talks to it over `TcpStream`, killing it in a `Drop` guard; it also checks the refusals of bad `--ui` and `--http-stdio` options |
| Script runner, exit code 1 on error | 00 | stdout is flushed before the error |
| The Windows console reads output as UTF-8 | 02 | `SetConsoleOutputCP(65001)`, a raw `extern "system"` declaration in `src/main.rs` rather than a crate, so `×` and non-ASCII text render. A pipe or a file is unaffected |
| Golden-file test harness | 00 | `tests/golden.rs`, no dependencies |
| Zero dependencies | 00 | And zero dev-dependencies |
