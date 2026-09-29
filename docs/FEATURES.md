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
| Command syntax | 04 | `command_disp_word`, `command_clear_two_words`, `command_variable_expression`, `err_command_clear_one`, `err_command_clear_all`, `hold_on_close_all_commands`, `format_long_then_short` (13), `grid_on_as_command` | MATLAB's rule: a statement that starts with a name that is not a variable, then whitespace, then a word that is not an operator followed by whitespace, calls the name with each word as a char argument. Quotes group words; the command ends at a newline, `,`, `;` or `%` outside quotes. `clear x y`, `clear all` and `disp hello` work; `x -1` with `x` a variable stays `x - 1`. Whether a name is a variable is decided before the source runs, from the workspace and the names it has assigned so far. Since cycle 12 `hold on`, `grid on` and `close all` call the plotting builtins, and since cycle 13 `format long`, `cd envdir`, `help sum` and `which sum` call theirs. It used to be a parse error (QA D31) |
| `...` line continuation | 00 | `demo_smoke` | Works straight after a digit, as in `a = 1...` |
| `...` separates elements inside brackets | 01b | `continuation_bracket_element` | `[1 ...` newline `-2]` is two elements, like `[1 -2]` |
| `;` suppresses display, `,` and newline show | 00 | `indexing` | |
| Matrix literals, space/comma/newline separators | 00 | `matrix_ops` | `[1 -2]` is two elements, `[1 - 2]` is one |
| Nested concatenation `[A; B]`, `[a' b']` | 00 | `builtins_sample` | |
| Ranges `a:b` and `a:s:b` | 00 | `ranges` | Descending and fractional steps |
| A range between two chars is a char | 11 | `char_range_and_diag_keep_char` | `'a':'e'` is `'abcde'`, and `diag('ab')` is a 2x2 char; both used to be doubles |
| A range that would not fit is a clean error | 01b | `err_range_too_large` | `1:1e15` used to abort in the allocator; same limit and wording as `check_shape` |
| A range lands exactly on its end point | 01d | `range_hits_end_point` | `x = 0:0.1:0.3; x(end) == 0.3` is `1`, and `-1:0.01:1` is symmetric: the upper half is computed from the right-hand end point, not by repeated addition |
| An infinite range end point is refused | 01d | `err_range_end_inf`, `err_range_start_neg_inf` | `0:Inf` and `-Inf:1:0` report `1xInf` rather than quietly giving a 1x0. `1:NaN` is still an empty, and still in Known bugs |
| An infinite range *step* follows the documented count | 01e | `range_infinite_step` | `1:Inf:5` is the 1x1 `1`: `fix((k-j)/i)` is `fix(4/Inf)`, which is `0`, and a count of `0` is one element. It used to be a 1x0. A range that runs against its step is still empty, `5:Inf:1` included |
| Chained ranges `1:2:3:4` | 01e | `err_chained_range` | Reads as `(1:2:3):4`, as MATLAB reads it; `parse_range` took at most two colons and did not loop, so it was a parse error. Both spellings then meet the same refusal, since a colon start that is not a scalar is an error here (Known bugs) |
| An unexpected character is named, not echoed | 11 | `err_lexer_control_character_named`, `err_lexer_printable_character_quoted` | A control or invisible character is written as its code point, `unexpected character U+0000`, so no raw byte reaches the terminal; a printable one is still quoted, `unexpected character '$'` |
| A leading UTF-8 byte-order mark is skipped | 01e | `bom_is_skipped` | The three bytes `EF BB BF` a Windows editor writes are an encoding marker, not source. A file that is not valid UTF-8 is now decoded leniently rather than refused, so a Windows-1252 comment runs; UTF-16 is still unread (Known bugs) |
| The tokens `{ }`, the field `.` and `@` | 03 | `err_brace_on_matrix`, `err_dot_on_matrix` | `c{1}`, `s.a` and `s.(n)` lex and parse, alone or chained (`c{1}(2).b`), without disturbing `1.5`, `.5`, `x.^2`, `x.*y`, `x./y`, `x.\y`, `x.'` or a `...` continuation. Inside brackets a brace or an `@` after a space starts an element. An `@` that starts no handle is `unexpected '@' in expression`. Since cycle 07 a `{` that does not follow a value opens a cell literal |
| An anonymous function's body inside `[]` or `{}` is one element | 06 | `err_handle_in_brackets` | The whitespace rule does not split the body: in `{@(x) x + 1, 2}` the body is `x + 1` and ends at the comma, as it would at a `;`, a newline or the closer. A quote straight after the parameter list opens a string, `@() 'hi'`. A handle cannot be an element of a bracket, so `[@(x) x+1]` is `Nonscalar arrays of function handles are not allowed; use cell arrays instead.`, as a parse error |
| Nesting is bounded, not unbounded | 01e | `err_nesting_parens`, `err_nesting_brackets`, `err_nesting_calls`, `err_nesting_flat_sum` | 10,000 levels of parentheses, brackets, calls, indexes, blocks or chained operators. Past that, a clean error from the parser and the identical one from the evaluator; about 96,000 levels used to abort the process with exit 134 (QA D4) |

## Operators

| Feature | Since | Golden case | Notes |
|---|---|---|---|
| `+ - * /` and left division | 00 | `matrix_ops`, `backslash_square_regression` | A square system goes through the shared LU since cycle 08 |
| Least squares `A \ b` and `b / A` | 08 | `backslash_least_squares`, `slash_least_squares` | A non-square system is solved in the least-squares sense by Householder QR with column pivoting; an underdetermined or rank-deficient one gets the basic solution, at most `rank` non-zero rows, and a rank-deficient one warns `Matrix is rank deficient to working precision (rank r).`, SplatCrab's own text. Before cycle 08 a non-square `\` was an error |
| A singular system warns | 08 | `singular_backslash_warns`, `singular_warning_protocol_out`, `singular_warning_protocol_in_order` | `[1 2; 2 4] \ [1; 2]` writes `Warning: Matrix is singular to working precision.` through `Interp.err` and returns what the substitution gives, `NaN NaN` here; under `--protocol` the warning is in the `eval`'s `out`, in order. It was an error until cycle 08 |
| The singular test is relative to the matrix | 01d | `solve_relative_pivot` | The pivot tolerance scales with the largest finite magnitude in the matrix, so the perfectly conditioned `[1e-15 0; 0 1e-15] \ [1; 1]` is solved rather than refused. A fixed `1e-14` used to judge it, and `det` and `\` disagreed on what singular means; they make the identical test, and since cycle 08 read the one LU |
| `^` with an integer exponent | 00 | `demo_smoke`, `mpower_singular_warns_inf` | Negative exponents invert; a singular matrix to a negative power warns and is `Inf` everywhere, as `inv` is (cycle 08) |
| `.* ./ .^` elementwise | 00 | `matrix_ops` | |
| `.\` elementwise left divide | 01b | `eldiv_vector`, `eldiv_after_number` | `a.\b` is `b./a`; `2.\x` no longer means `2 \ x` |
| Transpose `'` and `.'` | 00 | `matrix_ops`, `ctranspose_conjugates`, `transpose_does_not_conjugate` | Since cycle 10 `'` is the conjugate transpose and `.'` the plain one |
| `== ~= < <= > >=` | 00 | `logical_ops` | Results are `logical`, since 02; they were 0/1 doubles |
| `& \|` elementwise, `&& \|\|` short-circuit | 00 | `logical_ops` | `logical` results, since 02 |
| `~` negation | 00 | `logical_ops` | A `logical` result, since 02 |
| The class-propagation table | 02 | `class_propagation`, `concat_class` | Arithmetic, unary minus and unary plus give a double whatever their operands: `true + true` is `2`, `'a' + 1` is `98`, `+'a'` is `97`. Comparisons and the logical operators give a logical. Concatenation gives a char if any operand is one, else a logical only if every operand is: `['a' 66]` is `'aB'`, `[true 2]` a double. A 0x0 `[]` takes no part, so `[[] 'abc']` is a char |
| `&&` and `\|\|` need a logical scalar | 01e | `err_and_non_scalar`, `err_or_empty` | `[1 1] && 1` gave `1` and `[] \|\| 1` gave `1`; MATLAB errors, because the operators need one value to branch on. Short-circuiting is unchanged, so `0 && [1 1]` is still `0` and never looks at the right-hand side |
| A `NaN` cannot become a logical | 01e | `err_if_nan`, `err_and_nan`, `err_not_nan` | `if NaN` was taken as true, `NaN & 1` was `1` and `~NaN` was `0`. MATLAB and Octave both refuse: `NaN's cannot be converted to logicals.` The same conversion serves `if`, `while`, `&`, `\|`, `~`, `&&` and `\|\|` |
| Scalar and row/column broadcasting | 00 | `builtins_sample` | |
| A result too big to allocate is a clean error | 01d | `err_zip_result_size`, `err_matmul_result_size`, `err_matmul_size_wraps`, `err_index_result_size` | Broadcasting, `*`, a two-subscript read and a reduction all size their result from their operands, and all judge it before allocating. `ones(1e5,1) + ones(1,1e5)` used to abort the process, exit 134 |
| `*` keeps `Inf` and `NaN` through a zero factor | 01d | `matmul_keeps_inf_and_nan` | `[Inf 0] * [0; 1]` is `NaN`, as in MATLAB. A sparsity shortcut used to skip the multiply and give `0` |
| A result that would be complex is the complex value | 10 | `complex_sqrt_negative`, `complex_log_negative`, `complex_log2_negative`, `complex_log10_negative`, `complex_asin_above_one`, `complex_acos_outside_range`, `complex_power_operator_fractional`, `complex_elementwise_power_fractional`, `complex_power_builtin_fractional` | `sqrt(-4)`, `log(-1)`, `asin(2)`, `(-8)^(1/3)` and the rest used to return `NaN` and exit 0; cycle 01d made them a clean error, and cycle 10 the value on the principal branch. The nine `err_complex_*` cases of 01d went with the refusal |

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
| Brace and dot access on a matrix are errors | 03 | `err_brace_on_matrix`, `err_dot_on_matrix` | `x{1}` is `Brace indexing is not supported for variables of this type.` and `x.a` is `Dot indexing is not supported for variables of this type.`, reading; assigning is MATLAB's assignment form, `Unable to perform assignment because brace indexing is not supported for variables of this type.` and the same with `dot`, since cycle 07. A second `(...)` indexes the value so far, `x(2:3)(2)` |

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
| `switch` / `case` / `otherwise` | 04 | `switch_otherwise`, `switch_char_case`, `switch_cell_case`, `switch_break_in_for`, `err_switch_not_scalar`, `err_nesting_switch`, `err_switch_stray_statement`, `err_case_without_switch` | The first matching `case` runs, with no fall-through. A number matches a number of equal value whatever its class; a char matches a char of the same text and never a number by its code. `case {a, b}` matches any of its values, and is syntax rather than a cell. `break` and `continue` inside act on the enclosing loop. A subject that is neither a scalar nor a character vector is `SWITCH expression must be a scalar or a character vector.` |
| `try` / `catch` | 04 | `try_catch_message`, `try_catch_identifier`, `try_catch_undefined`, `try_rethrow_nested`, `rethrow_keeps_identifier`, `err_rethrow_uncaught`, `exception_class`, `err_exception_stack`, `err_exception_arithmetic`, `err_rethrow_not_exception`, `err_nesting_try` | Every runtime error inside `try` is caught, a builtin's included. `catch e` on the same line binds a minimal `MException`: `e.message`, `e.identifier`, `class(e)` is `'MException'`, `e.stack` since cycle 07 (see Cells and structs), and any other field is the Dot error. `catch` followed by a comma or a newline binds nothing, and a `try` with no `catch` ignores the error. `break` and `continue` pass through. `rethrow(e)` raises it again unchanged, line included |
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
| `@name` handles, bound where they are made | 06 | `named_handles`, `handle_local_from_path_file`, `handle_to_subfunction_returned`, `handle_passed_to_function` | `g = @sin; g(0)`. The name resolves when the handle is made, by invariant 4's order from there: a handle to a local function or a subfunction keeps calling it wherever it is called from. A name with no local function is looked up on the path and among the builtins when called |
| Anonymous functions `@(x) body` | 06 | `anonymous_call`, `capture_at_creation`, `nested_anonymous`, `err_capture_before_exists`, `err_handle_too_many_inputs` | Capture happens when the function is made: every name the body reads that is a variable then is snapshotted with its value; any other name is a function when the body runs. The call runs in a workspace of its own holding the parameters and the captures, with its own `end`, and counts against the recursion limit. One argument too many is `Too many input arguments.` |
| Calling a handle variable | 06 | `anonymous_call`, `handle_statement_ans`, `nargout_through_handle` | `f(args)`, `z()` with none; as a statement a value becomes `ans`. A body that is a single call is asked for the caller's `nargout`, so `f = @(v) max(v); [m, i] = f(v)` works and `@() disp(1)` is legal as a statement |
| Recursion through a handle is bounded | 06 | `err_handle_recursion_limit`, `err_feval_handle_recursion_limit`, `err_arrayfun_recursion_limit`, `err_anonymous_self_application`, `err_anonymous_feval_self_application`, `err_anonymous_arrayfun_self_application` | The same `Maximum recursion limit of 500 reached.` as a direct call. `feval` and `arrayfun` call back through `Interp::call_nested`, which counts each call against the shared nesting budget |
| A handle's display, `class` and `isa` | 06 | `handle_display`, `handle_class_isa`, `workspace_lists_handle` | `f = @(x) x + 1` shows `f =`, `  function_handle with value:` and `    @(x)x+1`; `disp(f)` prints `@(x)x+1`, and `disp(@sin)` prints `@sin`. `class` is `function_handle`, `isa(f, 'function_handle')` is a logical `1`, `who` and the protocol's `workspace` list it as `1x1 function_handle`. A handle is one function, not an array: `[@(x) x+1]` is a parse error (`err_handle_in_brackets`) and `[f 1]` a run-time one (`err_handle_concat_at_run_time`) |
| `feval arrayfun func2str str2func` | 06 | `feval_handle_and_name`, `arrayfun_uniform`, `err_arrayfun_size_mismatch`, `err_arrayfun_nonscalar_result`, `err_arrayfun_not_a_handle`, `func2str_str2func`, `func2str_brackets_round_trip`, `err_func2str_not_a_handle` | `feval` takes a handle or a name. `arrayfun` calls a handle on each element of one or more equal-size arrays and collects scalar results in their shape; `'UniformOutput', false`, since cycle 07, collects any results in a cell. `func2str` renders from the parse tree, with no spaces around operators and a comma between bracket elements, so `@(x) [x 1]` reads `@(x)[x,1]`. `str2func` takes a name or an `'@(...) ...'` text and captures nothing |

## Cells and structs

Cases in `07-cells-and-structs/`.

| Feature | Since | Golden case | Notes |
|---|---|---|---|
| Cell literals `{...}` and brace and paren indexing | 07 | `cell_literal_brace_and_paren`, `handle_in_cell_called` | A bracket's separators; each value is one element, so `{c}` nests a cell. `c{k}` reads the contents, `c(k)` gives a cell, and a chain goes on into the contents, `c{3}(2)`, `c{1}(3)` of a handle. Column-major, like a matrix |
| Cell growth, deletion and assignment | 07 | `cell_grow_and_delete`, `err_cell_paren_assign_double`, `err_matrix_paren_assign_cell` | `cell(n)`, `cell(r, c)`; `c{k} = v` past the end grows with `[]`; `c(k) = []` deletes; `c(k) = {v}` stores cells, and `c(k) = 5` is `Conversion to cell from double is not possible.` |
| Concatenation with a cell | 07 | `cell_concat_with_number`, `err_struct_concat_fields` | `[{1}, 2]` is a 1x2 cell, the number one element; structs with the same fields join into a struct array |
| Cell display | 07 | `cell_display`, `cell_deep_nest_display` | `  1×2 cell array`, then `    {[1]}    {'ab'}`: a scalar in brackets, a char row quoted, anything else its size and class; a nested cell is summarised, `{1×1 cell}`, never expanded, so the display of any depth is bounded. Columns pad to their widest element and wrap at 80 characters |
| Fields, dynamic fields and creation on assignment | 07 | `struct_nested_and_dynamic_field`, `dynamic_field_write`, `struct_many_fields_by_name`, `err_dot_assign_on_double`, `err_brace_assign_on_double`, `err_missing_field_read`, `err_invalid_dynamic_field_name`, `err_dynamic_field_not_char` | `s.a = 1` on an undefined `s` or on `[]` makes a struct; `s.inner.v = 3` creates the path; `s.(n)` names a field at run time. A field assignment into another value is `Unable to perform assignment because dot indexing is not supported for variables of this type.` |
| Struct arrays | 07 | `struct_array_cs_list`, `err_struct_assign_dissimilar`, `err_struct_array_field_assign` | `p(2).name = 'B'` grows `p`, every element with every field; `p(k)` is a struct |
| `struct` and the field functions | 07 | `struct_display`, `struct_fieldnames_isfield_rmfield`, `getfield_setfield`, `iscell_isstruct`, `err_struct_unpaired`, `err_struct_cell_sizes`, `err_fieldnames_not_struct` | `struct('a', 1, ...)`; a cell value makes a struct array of its size and a 1x1 cell is the one value of every element. `fieldnames` is a column cell, `isfield` takes a cell of names, `setfield` returns a copy |
| Struct display | 07 | `struct_display` | `s = `, then `  struct with fields:` and one `name: value` line per field, the names right-aligned; a struct array lists its field names |
| cs-lists | 07 | `cs_list_in_brackets`, `struct_array_cs_list`, `err_brace_cs_list_one_output` | `c{:}` and `p.name` spread into call arguments, `[ ]` and `{ }`; `[a, b] = c{:}` assigns in order; where one value is needed any other count is `Expected one output from a curly brace or dot indexing expression, but there were 2 results.` |
| `for` over a cell | 07 | `for_over_cell` | One column per iteration, a cell; over a struct array, a struct |
| `cellfun`, `arrayfun(..., 'UniformOutput', false)` | 07 | `cellfun_uniform`, `cellfun_uniform_output_false`, `arrayfun_uniform_output_false`, `err_cellfun_recursion_limit`, `err_cellfun_not_cell` | A handle, or for `cellfun` a function's name; each call through `call_nested`, so recursion through them is the clean limit |
| `num2cell`, `cell2mat`, `deal` | 07 | `num2cell_class`, `cell2mat_matrix`, `deal_one_input`, `err_cell2mat_nested_cell`, `err_deal_count` | `cell2mat` joins each row as a bracket would, then the rows |
| `varargin` and `varargout` | 07 | `varargin_nargin`, `varargout_two_outputs`, `err_varargout_not_cell`, `err_varargout_not_assigned` | The rest of the arguments as a 1xN cell (0x0 for none); the rest of the outputs from `varargout`'s elements. `nargin` and `nargout` count them. An anonymous function takes `varargin` too |
| `e.stack` | 07 | `exception_stack_fields`, `err_exception_stack` | An Nx1 struct array, `file`, `name` and `line`, one element per function frame, innermost first |
| Queries answer for every value | 07 | `handle_size_isempty`, `exception_isa_numel`, `is_predicates_every_value` | `size`, `numel`, `length`, `isempty`, `isscalar`, `isvector`, `isa`, `class`, `islogical`, `ischar` and `isnumeric` of a handle, an `MException`, a cell or a struct; a handle and an `MException` are 1x1 |
| Operators on a value that is not an array | 07 | `err_cell_operator_plus`, `err_struct_operator_times`, `err_handle_operator_plus`, `err_exception_operator_plus` | MATLAB R2020a's `Operator '+' is not supported for operands of type 'cell'.` for every binary operator; a unary operator keeps the generic text |
| Freeing never recurses | 07 | `cell_chain_freed`, `cell_handle_chain_freed`, `struct_chain_freed`, `chain_alive_at_exit` | A chain 500,000 deep through cells, structs and handles is freed from a worklist, cleared or alive at exit |
| The protocol lists cells and structs | 07 | `workspace_lists_cell_struct` | `class` `cell` or `struct` and the size, in U0's `vars` shape |

## Builtins

231 names, each an ordinary function in `src/builtins/` (the plotting ones
in `src/plot/`) registered by name in
`Interp::new`. Every one is exercised by `builtins_sample`, `reductions` or
`demo_smoke`, or for the class builtins by the cases in
`02-classes-and-display`; the shared-arm groups also by the `*_shared_arm`
cases in `01-registry-and-builtins`. Cycle 01 counted 81. Cycle 02 added the
eight class builtins, and cycle 04 five: `rethrow`, `lasterr`, `warning`,
`assert` and `isequal`, each exercised by the `04-switch-try-commands` cases.
Cycle 05 added six, `nargin`, `nargout`, `exist`, `feval`, `addpath` and
`rmpath`, exercised by the `05-functions-and-scoping` cases. Cycle 06 added three,
`arrayfun`, `func2str` and `str2func`, exercised by the `06-function-handles`
cases. Cycle 07 added thirteen, `cell`, `struct`, `fieldnames`, `isfield`,
`rmfield`, `getfield`, `setfield`, `iscell`, `isstruct`, `cellfun`,
`num2cell`, `cell2mat` and `deal`, exercised by the `07-cells-and-structs`
cases. Cycle 08 added fifteen, `lu`, `qr`, `chol`, `eig`, `svd`, `rank`,
`pinv`, `null`, `orth`, `cond`, `kron`, `cross`, `triu`, `tril` and `magic`,
exercised by the `08-linear-algebra` cases (see
[Linear algebra](#linear-algebra)). Cycle 09 added thirty-three, the
polynomials, samples, statistics, number theory, grids, sets and solvers,
exercised by the `09-numerics` cases (see [Numerics](#numerics)). Cycle 10
added ten, `i`, `j`, `real`, `imag`, `conj`, `angle`, `isreal`, `complex`,
`fft` and `ifft`, exercised by the `10-complex` cases (see
[Complex numbers](#complex-numbers)). Cycle 11 added thirty-eight, the
string, regular-expression and file functions, exercised by the
`11-strings-and-io` cases (see
[Strings, regular expressions and files](#strings-regular-expressions-and-files)).
Cycle 12 added twenty, the plotting builtins, exercised by the
`12-plotting` cases (see [Plotting](#plotting)). Cycle 01c removed `e`, which
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
| Factorisations | `lu qr chol eig svd rank pinv null orth cond` | 08 | `linalg.rs`, over `factor.rs` |
| Constructions | `kron cross triu tril magic` | 08 | `linalg.rs` |
| Search and sort | `find sort` | 00 | `linalg.rs`; `sort` of a matrix since 09 |
| Polynomials | `polyfit polyval roots conv deconv filter` | 09 | `numerics.rs`, over `factor.rs` |
| Samples | `interp1 trapz cumtrapz diff` | 09 | `numerics.rs` |
| Statistics | `std var median mode` | 09 | `numerics.rs` |
| Number theory | `factorial nchoosek primes isprime gcd lcm` | 09 | `numerics.rs` |
| Grids and counts | `logspace meshgrid histc` | 09 | `numerics.rs` |
| Sets | `unique ismember setdiff intersect union` | 09 | `sets.rs` |
| Solvers | `fzero fminsearch integral ode45 odeset` | 09 | `solvers.rs` |
| Output | `disp fprintf sprintf num2str` | 00 | `core.rs`; `printf.rs` the formatter, `fprintf` in `io.rs` and `num2str` in `strings.rs` since 11 |
| Strings | `strcat strsplit strjoin strrep strtrim upper lower strcmp strcmpi strncmp strncmpi strfind strtok int2str str2double str2num mat2str isspace isletter blanks` | 11 | `strings.rs` |
| Regular expressions | `regexp regexprep` | 11 | `strings.rs`, over `regex.rs` |
| Files | `input fopen fclose fgetl fgets fread fwrite feof fileread readmatrix writematrix csvread csvwrite delete save load` | 11 | `io.rs`, over `mat.rs` for MAT-files |
| Errors and warnings | `error rethrow lasterr warning assert` | 00, 04 | `core.rs` |
| Comparison | `isequal` | 04 | `core.rs` |
| Workspace | `clear clc who whos` | 00 | `core.rs`; `clear all` since 04; since 13 `who` is the names alone, `whos` a table with bytes, and `clc` writes only to a terminal |
| The environment | `cd pwd ls dir help which format eval evalc run datestr now clock pause getenv system version exit quit` | 13 | `environ.rs` |
| Timing | `tic toc` | 01 | `core.rs` |
| Functions and the path | `nargin nargout exist feval addpath rmpath` | 05 | `core.rs` |
| Function handles | `arrayfun func2str str2func` | 06 | `core.rs`; `feval` of a handle, `class` and `isa` of one since 06; `arrayfun` shares `cells.rs`'s map since 07 |
| Cells and structs | `cell struct fieldnames isfield rmfield getfield setfield iscell isstruct cellfun num2cell cell2mat deal` | 07 | `cells.rs` |

Reductions, and `cumsum` and `cumprod`, take an optional dimension argument;
a dimension past the array's returns the input unchanged and `0` is an error.
`sum`, `prod`, `mean`, `any` and `all` also take `'all'`, and so do `max` and
`min` as their third argument. `max` and `min` also take two arrays. `sort`
takes a matrix since cycle 09, sorting each column (or each row along
dimension 2), and `norm` since cycle 08. `sort` puts `NaN` last
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
| `sort(A)`, `sort(A, dim)`, `sort(A, 'descend')`, `[s, i] = sort(A)` of a matrix | 09 | `sort_matrix_columns`, `sort_matrix_along_rows`, `sort_matrix_descend`, `sort_matrix_permutation` | Each column on its own by default, each row along dimension 2; `i` is each slice's permutation. Was "'sort' currently supports vectors only." |
| `find(X, n)`, `find(X, n, 'first')`, `find(X, n, 'last')` | 01c | `find_count`, `err_find_count_zero`, `err_find_count_fraction`, `err_find_direction` | The last `n` stay in ascending order; `n` must be a positive integer |
| `norm(v, p)`: `1`, `2`, any `p > 0`, `Inf`, `-Inf`, `'fro'`, `'inf'` | 01c | `norm_order`, `err_norm_type` | A matrix takes its own norms since cycle 08 (see [Linear algebra](#linear-algebra)). `p = 0` and a negative finite `p` are refused |
| `norm` without overflow; an empty sum is `+0` | 01c | `norm_scaled_and_empty_sum` | `norm([1e200 1e200])` is `1.4142e+200`, not `Inf`. `sum([])`, `norm([])` and `dot([], [])` print `0.0000`, not `-0.0000` |
| `diag(v, k)` and `diag(A, k)` | 01c | `diag_offset`, `err_diag_offset` | A `k` past the matrix gives a 0x1 |
| `num2str(x, n)` and `num2str(x, formatSpec)` | 01c | `num2str_precision`, `err_num2str_precision` | `%.{n}g`, and `sprintf` with the leading spaces trimmed. Since cycle 11 a non-scalar gives one row per matrix row, with a format each row formatted on its own |
| `round(x, n)`, `round(x, n, 'decimals')`, `round(x, n, 'significant')` | 01c | `round_digits`, `err_round_digits`, `err_round_significant_digits`, `err_round_type` | Any integer `n`, ties away from zero. `round(pi, 20)` is `pi` and `round(5, -400)` is `0` |
| `max` and `min` of an empty follow MATLAB's rule | 01c | `max_min_empty_shape` | `max(zeros(3, 0))` is 1x0, `max(zeros(0, 3))` is 0x3 |
| `dot(A, B)` of matrices, and `dot(A, B, dim)` | 01c | `dot_matrices`, `err_dot_sizes`, `err_dot_vector_length`, `err_dot_dim_orientation` | Column-wise; two vectors of equal length may differ in orientation, but not with `dim` |
| `any` ignores `NaN` | 01c | `any_ignores_nan` | `any(NaN)` is `0`, as the MATLAB page says, and GNU Octave 8.4 agrees. `all(NaN)` is `1` |
| `isvector` of a 1x0 or a 0x1 is true | 01c | `isvector_empty` | A 0x0 is not a vector |
| A bare `toc` needs an earlier bare `tic` | 01c | `toc_after_bare_tic`, `err_toc_before_tic`, `err_toc_value_before_tic`, `err_toc_after_handle_tic` | `t = tic` does not count; `toc(t)` is unaffected |
| `'all'`, and no char is ever a dimension | 01c | `reduction_all_option`, `err_reduction_char_dim`, `err_cumsum_all`, `err_max_char_dim`, `err_size_char` | `sum(A, 'x')` used to reduce along dimension 120 |

## Linear algebra

Cycle 08. The numerics are in `src/builtins/factor.rs`, the builtins over
them in `linalg.rs`; the Design notes of `docs/modules/08-linear-algebra.md`
record every choice.

| Feature | Since | Golden case | Notes |
|---|---|---|---|
| One LU for `det`, `inv`, `\`, `/`, `A^-n` and `lu` | 08 | `lu_three_outputs`, `backslash_square_regression` | Partial pivoting on the first largest element, as `idamax` picks it. `[L, U, P] = lu(A)` has `P*A = L*U`, `[L, U] = lu(A)` the permuted `L`, `Y = lu(A)` LAPACK's packed form; any `m`-by-`n` `A`. The arithmetic is the elimination `det` and `\` made before, so `det([1 2; 3 4])` is still exactly `-2` |
| `inv` and `A^-1` of a singular matrix | 08 | `inv_singular_warns_inf`, `inv_zero_warns_inf`, `mpower_singular_warns_inf` | The singular warning and `Inf` everywhere: `inv([1 2; 2 4])` is `Inf Inf; Inf Inf` and `inv(0)` is `Inf`, as in MATLAB and Octave (QA D26). A matrix singular only to working precision, with no exact zero pivot, warns and returns the computed inverse |
| `[Q, R] = qr(A)` | 08 | `qr_factors` | Householder, full: `Q` is `m`-by-`m`. `R`'s diagonal takes LAPACK's signs, so it may be negative; below it is exact `0`. `R = qr(A)` gives `R` alone |
| `chol(A)`, `[R, p] = chol(A)` | 08 | `chol_upper_factor`, `err_chol_not_positive_definite`, `err_chol_nan_input` | Upper `R` with `R'*R = A`, from the upper triangle. Not positive definite, a `NaN` pivot included, is `Matrix must be positive definite.`; the two-output form returns the failing column instead |
| `eig(A)`, `[V, D] = eig(A)` | 08 | `eig_symmetric_ascending`, `eig_nonsymmetric_real`, `eig_nonsymmetric_hessenberg`, `eig_two_outputs`, `eig_vectors_residual`, `eig_complex_eigenvalues_rotation`, `err_eig_nan_input` | An exactly symmetric `A` by cyclic Jacobi, eigenvalues ascending; any other by Hessenberg reduction and the shifted double QR iteration, in the order found, unit eigenvectors. Since cycle 10 a complex pair is two complex values, the positive imaginary part first, with complex vectors (it was a refusal, `err_eig_complex_eigenvalues`); a complex `A` is refused. A `NaN` or `Inf` is `Input to 'eig' must not contain NaN or Inf.` |
| `svd(A)`, `[U, S, V] = svd(A)` | 08 | `svd_values_descending`, `svd_three_outputs`, `svd_zero_row_converges`, `err_svd_inf_input` | One-sided Jacobi; values descending; `U` and `V` full and orthogonal. A `NaN` or `Inf` is refused, as for `eig` |
| `rank pinv null orth cond` | 08 | `rank_deficient_and_full`, `pinv_rank_deficient`, `null_basis`, `orth_basis`, `norm_matrix_and_cond` | Through the SVD. `rank` counts singular values above `max(m, n) * eps(s(1))`, and `null` and `orth` use the same tolerance; `pinv` drops those at or below `max(m, n) * s(1) * eps`. `rank` and `pinv` take a tolerance argument. `cond` is `s(1) / s(end)`, `Inf` for a singular matrix |
| Matrix `norm` | 08 | `norm_matrix_and_cond`, `err_norm_matrix_type`, `err_norm_matrix_order` | `norm(A)` and `norm(A, 2)` the largest singular value, `norm(A, 1)` the largest column sum, `norm(A, Inf)` the largest row sum, `norm(A, 'fro')` the root sum of squares. Other orders are `Matrix norm type for 'norm' must be 1, 2, Inf or 'fro'.` A `NaN` gives `NaN`; an `Inf` gives an `Inf` 2-norm |
| `kron cross triu tril magic` | 08 | `kron_two_vectors`, `cross_unit_vectors`, `triu_tril`, `magic_three` | `cross` works along the first dimension of length 3 of two same-sized arrays; `triu` and `tril` keep the class; `magic(n)` is MATLAB's construction for odd, doubly even and singly even `n` |
| Every iteration terminates | 08 | `err_eig_nan_input`, `err_svd_inf_input`, `err_chol_nan_input` | The Jacobi sweeps (100), the SVD sweeps (100) and the QR iteration (`30 * max(10, n)` per eigenvalue) have caps, past which the answer is `'eig' did not converge within its iteration limit.` (or `'svd'`); no input known reaches one. `eig`, `svd`, `rank`, `pinv`, `null`, `orth` and `cond` refuse a `NaN` or `Inf`; `lu`, `qr`, `det`, `inv` and `\` let it spread to a `NaN` result |

## Numerics

Cycle 09. The builtins on data are in `src/builtins/numerics.rs`, the set
functions in `sets.rs` and the solvers in `solvers.rs`; the Design notes of
`docs/modules/09-numerics.md` record every choice. A function that works
along a dimension takes MATLAB's default, the first that is not a
singleton, so a matrix is worked on column by column.

| Feature | Since | Golden case | Notes |
|---|---|---|---|
| `polyfit(x, y, n)`, `polyval(p, x)` | 09 | `polyfit_line_fit`, `polyval_scalar` | Least squares through the column-pivoted QR of cycle 08; too few distinct points warns with the rank-deficient text. `[p, S, mu]` is not provided |
| `roots(p)` | 09 | `roots_real_distinct`, `roots_complex_pair_in_order`, `roots_complex_sorted` | Eigenvalues of the companion matrix, a column, then a `0` per trailing zero. Since cycle 10 complex roots are complex values, and a real root of multiplicity three or more comes back as a complex pair a rounding error apart, as the eigensolver finds it (a refusal before, `err_roots_complex`); complex coefficients are refused |
| `conv(u, v, shape)`, `[q, r] = deconv(b, a)` | 09 | `conv_polynomials`, `deconv_quotient_remainder` | `'full'`, `'same'` and `'valid'`. `r` has the shape of `b`, its leading coefficients exactly `0` |
| `filter(b, a, x)` | 09 | `filter_iir_impulse`, `filter_fir_moving_average` | Direct form II transposed from rest, normalised by `a(1)`, along the first non-singleton dimension; `zi`, `zf` and `dim` are not provided |
| `interp1(x, v, xq, method, extrap)`, `interp1(v, xq)` | 09 | `interp1_linear_inside`, `interp1_nearest`, `interp1_outside_is_nan` | `'linear'`, `'nearest'` (a tie takes the upper point), `'previous'`, `'next'`; `NaN` outside unless `'extrap'` or a value. `'spline'` and `'pchip'` are refused |
| `trapz`, `cumtrapz` | 09 | `trapz_unit_spacing`, `trapz_with_x`, `cumtrapz_unit_spacing` | `(y)`, `(x, y)`, `(y, dim)`, `(x, y, dim)`; `x` a spacing or the points |
| `diff(X, n, dim)` | 09 | `diff_vector`, `diff_second_order`, `diff_matrix_columns` | Without `dim`, each round works along the first non-singleton dimension of what the last left |
| `std var median mode` | 09 | `std_var_sample`, `median_odd_count`, `mode_most_frequent` | Column-wise, with a dimension; `std(X, 1)` normalises by `N`. `median` of a `NaN` is `NaN`; `mode` ignores `NaN`, takes the smallest of a tie, and gives the count second |
| `factorial nchoosek primes isprime gcd lcm` | 09 | `factorial_five`, `factorial_overflow_inf`, `nchoosek_five_two`, `primes_to_twenty`, `isprime_row`, `gcd_scalars`, `lcm_scalars`, `err_primes_huge_bound` | `nchoosek(v, k)` lists the combinations; `isprime` is Miller-Rabin; `gcd` and `lcm` take integers of either sign |
| `logspace meshgrid histc` | 09 | `logspace_decades`, `meshgrid_size`, `meshgrid_grid_values`, `histc_bin_counts`, `err_logspace_huge_count`, `err_meshgrid_huge_size` | `logspace(a, pi)` ends at `pi`; `[n, bin] = histc(x, edges)` |
| `unique ismember setdiff intersect union` | 09 | `unique_sorted_row`, `ismember_tf_loc`, `setdiff_sorted_row`, `intersect_sorted_row`, `union_sorted_row` | Sorted, or `'stable'`; index outputs `[C, ia, ic]`, `[tf, loc]`, `[C, ia]` and `[C, ia, ib]`, each a column. `NaN` is never equal to itself |
| Cells of char in the set functions | 09 | `unique_cellstr`, `unique_cellstr_char_code_order`, `ismember_char_in_cellstr`, `ismember_cellstr_two_outputs`, `setdiff_cellstr`, `intersect_cellstr`, `union_cellstr`, `err_unique_cell_of_numbers`, `err_ismember_cell_of_numbers` | Sorted by character code; a character vector beside a cell is one word; any other cell content is refused |
| `fzero(f, x0)`, `fzero(f, [a b])` | 09 | `fzero_scalar_start`, `fzero_bracket`, `fzero_nested_in_fzero`, `err_fzero_no_sign_change` | MATLAB's outward search for a sign change, then Brent's method to `2 eps abs(x)`. No sign change is a clean error, exit 1 |
| `fminsearch(f, x0)` | 09 | `fminsearch_quadratic_bowl`, `err_fminsearch_unbounded` | Nelder-Mead with MATLAB's simplex, coefficients and tolerances; `200 * numel(x0)` iterations and evaluations, past which it is an error |
| `integral(f, a, b)` | 09 | `integral_finite_interval`, `integral_infinite_limits`, `err_integral_divergent` | Global adaptive Gauss-Kronrod 7-15, `AbsTol` 1e-10 and `RelTol` 1e-6; an infinite limit is mapped onto a finite interval; 650 subintervals at most |
| `ode45(f, tspan, y0, opts)`, `odeset` | 09 | `ode45_scalar_decay`, `ode45_oscillator_system`, `ode45_output_columns`, `odeset_tolerances`, `err_ode45_blowup` | Dormand-Prince 5(4) with MATLAB's step control and `Refine` 4; `RelTol`, `AbsTol`, `MaxStep`, `InitialStep`, `Refine`. 50,000 steps at most, and a step below `16 eps(t)` is an error |
| Every solver call is counted | 09 | `err_fzero_recursion_limit`, `err_fminsearch_recursion_limit`, `err_integral_recursion_limit`, `err_ode45_recursion_limit` | Through `Interp::call_nested`, so a function that calls a solver on itself meets the recursion limit, a clean error |

## Complex numbers

Cycle 10. `Matrix` holds the imaginary parts in `im`, column-major like
`data`; the scalar arithmetic, `fft` and `ifft` are in
`src/builtins/complex.rs`, and the Design notes of
`docs/modules/10-complex.md` record every choice.

| Feature | Since | Golden case | Notes |
|---|---|---|---|
| Imaginary literals `1i`, `2.5j`, `1e3i` | 10 | `imaginary_literal_forms`, `class_of_complex_is_double` | A number with `i` or `j` straight after it; a complex value is class `double` |
| `i` and `j` as the unit, shadowed by a variable | 10 | `imaginary_unit_shadowed_by_variable` | Builtins, so invariant 4 puts a variable first, and `clear i` brings the unit back |
| Complex display | 10 | `display_complex_scalar`, `abs_real_imag_conj` | `   3.0000 - 4.0000i`, four decimals on both parts, a zero as `0.0000`; columns of one width; a scale factor or `e` format outside `[0.01, 1000)`; wide matrices wrap in column blocks |
| The flag rule | 10 | `isreal_arithmetic_drops_zero_imag`, `isreal_complex_keeps_zero_imag` | MathWorks': a result whose imaginary parts are all zero is real, `isreal(1i * 0)` is true, and `complex(a, b)` keeps complex storage, `isreal(complex(1, 0))` false. Indexing, concatenation and deletion follow the same rule; an indexed assignment keeps complex storage until the next arithmetic result (`assign_keeps_complex_storage`), which keeps assignment loops linear (`complex_growth_perf_guard`) |
| `+ - * / \ .* ./ .\ .^ ^`, unary minus | 10 | `complex_arithmetic_operators`, `complex_index_and_assign` | Matrix products, `\` and `/` of complex systems through the real embedding, integer matrix powers; a real target assigned a complex value becomes complex |
| `real imag conj angle abs isreal complex` | 10 | `abs_real_imag_conj`, `angle_of_complex`, `isreal_complex_keeps_zero_imag` | `angle` is in `[-pi, pi]` |
| Comparisons | 10 | `eq_compares_both_parts`, `ne_compares_both_parts`, `ordering_compares_real_parts`, `le_ge_compare_real_parts` | MathWorks': `==` and `~=` both parts, `< <= > >=` the real parts only |
| `fprintf` and `sprintf` print the real part | 10 | `printf_prints_real_part`, `sprintf_prints_real_part` | MathWorks' `sprintf` page: "Numeric conversions print only the real component" |
| `sum prod mean cumsum exp sin cos sqrt log log2 log10 asin acos power` | 10 | `complex_kernels_take_complex_input`, `sin_cos_imaginary_axis`, `complex_sqrt_negative` and the rest of the row above | Complex input as well as complex results; the branch cuts are on the negative real axis and, for `asin` and `acos`, outside `[-1, 1]`, a point on a cut taken from above |
| Complex `eig` and `roots` | 10 | `eig_complex_eigenvalues_rotation`, `roots_complex_pair_in_order`, `roots_complex_sorted` | See the rows under [Linear algebra](#linear-algebra) and [Numerics](#numerics) |
| `fft` and `ifft` | 10 | `fft_impulse`, `fft_moduli_length_four`, `ifft_round_trip_length_three`, `fft_prime_length_fast` | Of a row, or of each column; radix-2 Cooley-Tukey for a power of two and Bluestein's chirp convolution for any other length, so every length is O(n log n): a prime length of 100,003 runs at once |
| A builtin that does not take complex input refuses it | 10 | `err_sort_complex_input`, `err_max_complex_input`, `err_min_complex_input`, `err_floor_complex_input`, `err_mod_complex_input`, `err_fzero_complex_start`, `err_fzero_complex_value`, `err_if_complex_condition`, `err_colon_complex_operand`, `err_char_target_complex_assign` | `Complex values are not supported by 'sort'.`, from the registry's gate for every builtin not on `builtins::TAKES_COMPLEX`, and from a solver for a complex value its function returns; `if`, `while`, `&`, `\|`, `~`, `&&` and `\|\|` say `Complex values cannot be converted to logicals.`, a complex operand of `:` is refused as `':'`, and a complex value assigned into a char array as `'char'`. Never an answer from the real parts alone |
| Complex values in cells, structs and the workspace | 10 | `cell_and_struct_hold_complex`, `workspace_lists_complex_as_double` | Under `--protocol`, `workspace` lists a complex variable with class `double`, in U0's `vars` shape |

## Strings, regular expressions and files

Cycle 11. The string functions are in `src/builtins/strings.rs`, the
regular-expression engine in `regex.rs`, the formatter in `printf.rs`, the
file functions in `io.rs` and the MAT-file reader and writer in `mat.rs`.
Cases in `tests/cases/11-strings-and-io/`.

| Feature | Since | Golden case | Notes |
|---|---|---|---|
| `strcat` | 11 | `strcat_trailing_space` | A char argument loses its trailing whitespace, where `[...]` keeps it; with a cell among the arguments the result is a cell and nothing is trimmed |
| `strsplit` and `strjoin` | 11 | `strsplit_strjoin` | Whitespace by default; a run of delimiters collapses into one unless `'CollapseDelimiters'` is false; a delimiter may be a cell, and its escapes are processed. `[C, matches] = strsplit(...)` |
| `strrep`, `upper`, `lower`, `strtrim`, `strfind` | 11 | `strrep_upper_strtrim_strfind`, `string_lower_strncmp_isspace_blanks` | `strrep` and `strfind` count overlapping occurrences, as MATLAB does: `strrep('2222', '22', '*')` is `'***'`. Each maps over a cell of text |
| The `strcmp` family | 11 | `strcmp_family_logical`, `string_lower_strncmp_isspace_blanks` | `strcmp`, `strcmpi`, `strncmp` and `strncmpi` return logicals; a cell compares element by element, and anything that is not text compares false |
| `strtok`, `isspace`, `isletter`, `blanks` | 11 | `regexprep_strtok`, `string_lower_strncmp_isspace_blanks` | `[tok, rest] = strtok(s)`: the remainder keeps its leading delimiter |
| `num2str` of a matrix (QA D13) | 11 | `num2str_matrix_rows`, `str2double_str2num_num2str_int2str` | One char row per matrix row: `num2str([1 2; 3 4])` is the 2x4 `'1  2'` / `'3  4'`, where it was the 1x10 `'1  3  2  4'`. An integer column is its widest magnitude plus two wide; any other element is `%g` at the significant digits of the largest magnitude, in a column seven wider, one more with a negative element |
| `int2str`, `mat2str`, `str2double`, `str2num` | 11 | `str2double_str2num_num2str_int2str`, `mat2str_sprintf_fprintf_percent`, `str2num_constants_ignore_the_path` | `mat2str` is `%.15g` MATLAB syntax; `str2double` is `NaN` for anything but a number's text; `str2num` reads literals and operators only, never a call or a variable, and its constants (`pi`, `Inf`, `NaN`, `eps`, `true`, `false`, `i`, `j`) are the builtins' values, never a file on the path of that name |
| The string functions refuse a complex argument | 11 | `err_num2str_complex_input`, `err_mat2str_complex_input` | Cycle 10's gate: `Complex values are not supported by 'num2str'.` |
| `regexp` and `regexprep` | 11 | `regexprep_strtok`, `err_regexprep_long_replacement` | Outputs by name in the order given, `'once'`, `'ignorecase'`, `'emptymatch'`; `$0`, `$N` and `$<name>` in a replacement, read in linear time, and a result sized by its tokens before it is built. The syntax is listed in the spec's Design notes |
| The regular-expression engine runs in linear time | 11 | `regexp_nested_star_linear_time`, `err_regexp_class_too_large` | A Pike VM: `regexp(repmat('a', 1, 1e5), '(a*)*b', 'match')` returns at once. All the matches are found in one pass, so a pattern whose preferred branch runs far past each match is linear too. A class is built once into sorted ranges and bisected, whatever repeats it, and its ranges count against the program's size |
| A backreference is refused | 11 | `err_regexp_backreference` | `Backreferences are not supported in regular expressions.`; so are lookaround, atomic groups, possessive quantifiers, conditionals and inline flags, each with a message of its own |
| `input` | 11 | `input_number_and_string`, `err_input_refused_under_protocol`, `err_input_refused_under_http_stdio` | `input(prompt)` evaluates the line, `input(prompt, 's')` returns it; the prompt has no newline after it. Under `--protocol`, `--ui` and `--http-stdio` it is refused before the prompt, since standard input is the protocol's or there is no terminal |
| `fopen`, `fclose`, `fgetl`, `fgets`, `feof` | 11 | `fopen_fprintf_fgetl`, `fgets_feof` | Identifiers from 3 up, the lowest free first; `fopen` of a file it cannot open is `-1` and a reason; `fgetl` and `fgets` are `-1` at the end |
| `fprintf(fid, ...)` and its byte count (QA D25) | 11 | `fprintf_fid1_and_byte_count`, `fprintf_fid2_to_stderr`, `err_fprintf_invalid_fid` | `1` is stdout and `2` stderr, through `Interp.err`, so under `--protocol` it lands in `out`; `n = fprintf(...)` is the number of bytes; an identifier that names no open file is `Invalid file identifier.` |
| `fread` and `fwrite` | 11 | `fwrite_fread_round_trip` | `uint8` by default, and the integer, `single` and `double` precisions; `'*char'` reads chars |
| `fileread`, `readmatrix`, `writematrix`, `csvread`, `csvwrite` | 11 | `writematrix_readmatrix_fileread`, `csvwrite_csvread` | `writematrix` writes 15 significant digits, `csvwrite` five; the size of a matrix read from text is judged before it is allocated |
| `delete` | 11 | `err_delete_wildcard`, `delete_and_load_missing_warn` | One or more files, each by its full name; a file that is not there is the warning `File 'x' not found.` and the others are still deleted; a name with a wildcard is `Wildcards are not supported by 'delete'.` before anything is deleted, rather than expanded, a recorded deviation |
| `save` and `load` | 11 | `save_load_mat_round_trip`, `save_load_ascii`, `err_save_fieldless_struct_bound` | Uncompressed MAT-files of version 5: doubles (complex too), logicals, chars, cells and structs, and on reading also the integer and `single` classes, as doubles. `-ascii` writes MATLAB's `%.7e` columns. `S = load(...)` gives a struct. `save` refuses a variable past the 4 GiB an element can hold and a struct array with no fields past 1,048,576 elements, which `load` would refuse |
| `load` treats a MAT-file as untrusted | 11 | `err_load_mat_truncated`, `err_load_mat_huge_dims`, `err_load_mat_fieldless_struct_bound` | Every length is checked against the bytes there and every size goes through `check_shape` before anything is allocated, so a truncated file is `Unable to read MAT-file ...` and a header claiming 1e12 elements is `Requested 1000000x1000000 array exceeds the maximum array size.` A struct array with no fields, which its bytes cannot bound, may have at most 1,048,576 elements (`mat::MAX_FIELDLESS`), a recorded deviation |
| A file path resolves against the current folder | 11 | every file case | `fopen`, `fileread`, `readmatrix`, `writematrix`, `csvread`, `csvwrite`, `save`, `load` and `delete` all go through `Interp::cwd` |

## Plotting

Cycle 12. The builtins are in `src/plot/mod.rs`, the figure state, the
layout and the tick rule in `figure.rs`, the SVG writer in `svg.rs`, and
the rasterizer, its bitmap font and the PNG encoder in `png.rs`. Cases in
`tests/cases/12-plotting/`, which read a saved SVG back with `fileread`
and `strfind` and delete every file they write.

| Feature | Since | Golden case | Notes |
|---|---|---|---|
| `figure`, `gcf`, `close`, `clf` | 12 | `gcf_counts_figures`, `close_all_restarts_numbering`, `gcf_class_double`, `limits_axis_clf_run`, `err_close_not_open` | `figure` makes the lowest unused number current, `figure(n)` makes figure `n` current, made if need be, and `n = figure` returns the number. `gcf` returns the current figure's number as a double, making figure 1 if none is open: MATLAB before R2014b, a recorded deviation, since MATLAB now returns a Figure object. `close` closes the current figure, `close(n)` figure `n`, and `close all` every figure; a number that names no open figure is `Invalid figure handle.` `clf` removes every axes |
| `subplot(m, n, p)` | 12 | `subplot_two_axes`, `err_subplot_index_past_grid` | `p` counts along the rows from the top left; a vector `p` spans the cells, and `subplot(211)` is `subplot(2, 1, 1)`. The axes at a place is reused, and a new one deletes every axes it overlaps, as MATLAB's does; each axes is a `class="axes"` group of the SVG |
| `plot` | 12 | `plot_polyline_and_tick_label`, `plot_matrix_columns`, `plot_line_spec_red_dashed`, `hold_on_two_lines`, `err_plot_bad_line_spec`, `err_plot_lengths_differ` | `plot(y)` against `1:n`, a matrix one line per column; `plot(x, y)` pairing a vector with the matrix columns (or rows) of its length; several `x, y, spec` groups in one call. Line specs: a colour `rgbcmykw`, a style `-`, `--`, `:`, `-.` and a marker `o+*.xsd^v><ph`, in any order; a marker alone draws no line. Series without a colour take MATLAB's colour order in turn. A line is one `<polyline>`, split at a `NaN` or `Inf` |
| `scatter` | 12 | `scatter_circles`, `err_scatter_negative_size`, `err_scatter_bad_color` | `scatter(x, y, sz, c, 'filled')`: a `<circle>` a point; `sz` in points squared, one or one a point (default 36); `c` a colour letter or an RGB triple |
| `bar` | 12 | `bar_rects`, `err_bar_text_data`, `err_bar_zero_width` | `bar(y)`, `bar(x, y)`, `bar(y, width)`, `bar(x, y, width)` and a colour letter; a matrix is grouped, one series per column. Each bar is a `<rect class="bar">` |
| `histogram` | 12 | `histogram_bins`, `err_histogram_zero_bins` | `histogram(x, n)` in `n` equal bins from the smallest value to the largest, the last closed; `histogram(x, edges)`; with neither, Sturges' rule, a recorded choice. Non-finite values are left out |
| `xlabel`, `ylabel`, `title`, `legend` | 12 | `labels_title_legend_text`, `labels_escaped_in_svg` | Each text is a `<text>` of its own, escaped, so `xlabel('t')` is `>t<` in the SVG. `legend` takes labels or a cell, `legend off` removes it, and a label with no series to name is not drawn. No TeX: `_` and `^` are literal |
| `grid`, `hold`, `axis`, `xlim`, `ylim` | 12 | `grid_on_as_command`, `hold_on_two_lines`, `limits_axis_clf_run`, `err_hold_unknown_option`, `err_xlim_decreasing` | `on`, `off` or a toggle; `hold all` is `hold on`. `axis([x0 x1 y0 y1])`, `axis tight`, `auto`, `equal`, `image`, `square`, `normal`, `manual`, `on` and `off`; `v = axis`, `v = xlim` and `v = ylim` return the limits shown, and an infinite limit is automatic. With hold off, a plotting call clears the axes and resets its labels, legend, grid and limits, MATLAB's `NextPlot` `'replace'` |
| Tick labels | 12 | `plot_polyline_and_tick_label` | A step of 1, 2 or 5 times a power of ten, at most ten intervals; automatic limits widen to multiples of it. A label is written from its digits, so `9` is `9` and `0.3` is `0.3` |
| `saveas` and `print` | 12 | `print_png_signature`, `err_print_huge_resolution`, `err_saveas_unsupported_format`, `err_saveas_no_extension`, `err_print_bad_resolution`, `err_print_no_file_name` | `saveas(fig, 'f.svg')`, `saveas(fig, 'f.png')` or `saveas(fig, 'f', 'png')`, and a name with no extension and no format is an error that says how to give one; `print('-dpng', '-r150', 'f.png')`, `print('-dsvg', 'f.svg')`, `-f2` or a leading figure number. A PNG is 560x420 at the default 96 dots an inch, and its pixel size goes through `check_shape`, so `-r100000` is a clean `Requested ...` error before any file is written. Paths resolve against the current folder |
| Complex data is refused | 12 | `err_plot_complex_input` | Cycle 10's gate: `Complex values are not supported by 'plot'.`; MATLAB plots the real part against the imaginary, a recorded deviation |
| Bounded work | 12 | `plot_large_linear`, `err_plot_too_many_points`, `err_plot_markers_over_budget`, `extreme_limits_stay_numbers` | A plotting call copies its data and nothing more; drawing is one pass over the points, and each segment the rasterizer walks is clipped first. A figure holds at most 16,777,216 points: a line's vertex counts one, and a marker (6 for `o` to 17 for a hexagram), a scatter circle (6), a bar (9), each run of a line and each series what its SVG takes, so no figure writes much more than 256 MiB of SVG. The count is judged from the arguments, read in place, before the call copies or changes anything |
| Figures in memory | 12 | unit test in `src/plot/mod.rs` | `Interp::figure_numbers()` and `Interp::figure_svg(n)`, the SVG `saveas` would write, for a front end to show inline |
| The REPL's viewer | 12 | by hand | Only when standard input is a terminal: after each entry, every figure it changed is written to `splatcrab-<pid>-figure-<n>.svg` in the temporary folder and opened with `cmd /c start`, `open` or `xdg-open`, a failure ignored. Scripts, golden cases, CI, `--protocol`, `--ui` and `--http-stdio` never open one |

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
| `who` and `whos` show the class | 02 | | A logical is `logical` and a char has its real shape, `2x2 char`; they used to be `double` and `1xN char`. Since cycle 13 the class is `whos`'s alone |
| `format short` and `format long` | 13 | `tests/cases/13-environment/`; `interp.rs` `format_long_display` | Interpreter state; `format` alone is `short`. `long` keeps every layout rule of `short` and writes fifteen decimals, in columns three wider than the widest element: `format long; disp(pi)` is `   3.141592653589793`. Integers and logicals are unchanged. Any other format is a clean error |
| `disp([])` prints nothing | 01e | `empty_result_shapes` | It used to print `     []`. `disp('')` is still a line with nothing on it |
| Empty results have MATLAB's shapes | 01e | `empty_result_shapes` | `find([])` and `diag([])` are `0x0`, not `0x1`; `size('')` is `0 0`, not `1 0`, and `num2str([])` follows it. A shape with an orientation to keep still keeps it: `find([0 0])` is `1x0` |
| `fprintf` and `sprintf` | 00 | `fprintf_vector` | `%d %i %u %f %F %e %E %g %G %x %X %o %c %s`, flags, width, precision |
| The rest of `printf` (QA D16) | 11 | `printf_upper_exponent_and_s_of_fraction`, `printf_hash_flag_and_zero_pad_nonfinite`, `printf_escapes_hex_octal_control`, `printf_hex_octal_and_star`, `printf_invalid_conversion_truncates`, `err_printf_star_width_bounded`, `err_printf_star_precision_bounded` | `%E` and `%G` write an upper-case `E`; `%s` of a non-integer is `%e`, the MATLAB page's `3.141593e+00`; `#` keeps the point, `sprintf('%#.0f', 3)` is `3.`; `0` pads `Inf` and `NaN` with spaces; `\xN`, `\N` (octal), `\a`, `\b`, `\f` and `\v` are processed; `%x`, `%X`, `%o` and a `*` width or precision exist, the `*` ones bounded like written ones; an invalid conversion or a trailing `%` ends the output, `sprintf('abc%q', 1)` being `abc`. The formatter moved to `printf.rs` |
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

## Environment

Cycle 13. Every golden case is in `tests/cases/13-environment/`; the unit
tests are named beside each row.

| Feature | Since | Tests | Notes |
|---|---|---|---|
| The current folder is the interpreter's | 13 | `interp.rs` `cd_moves_the_interpreter_not_the_process` | `cd`, `pwd`, `ls`, `dir`, `run`, `system`, every file builtin and every path lookup use `Interp::cwd`; `cd` never moves the process. `cd ..` and `.` are resolved by their components, so `pwd` never shows them. `cd nope` is `Cannot CD to nope (Name is nonexistent or not a directory).` |
| `ls` and `dir` | 13 | `environ.rs` `wildcards_match_whole_names` | One name per line in byte order, `.` and `..` left out; a folder, a file or a `*`/`?` pattern. `s = ls` is a padded char matrix, `s = dir` an Nx1 struct of `name`, `folder`, `bytes` and `isdir` |
| `help` and `which` | 13 | `environ.rs` `the_help_block_is_the_leading_comments` | `help sum` is the registry's help line; `help myf` a file's leading `%` block, before or just after its `function` line, each line less its `%`. `which sum` is `built-in (sum)`, a file its full path, a variable `x is a variable.`, anything else `'x' not found.` |
| `who` and `whos` | 13 | `core.rs` `who_names_and_whos_bytes` | `who` is `Your variables are:` and the names, wrapped at 80 columns. `whos` is one row per variable, `  x            1x3   24  double`: 8 bytes a double element, 16 complex, 1 a logical, 2 a char, a cell or struct the sum of what it holds, a handle 0; no heading row |
| `eval`, `evalc`, `run` | 13 | `interp.rs` `exit_passes_every_frame_and_eval_runs_text`, `eval_goes_through_the_nesting_budget` | `eval(code)` runs statements in the workspace, `v = eval(expr)` evaluates one expression, `eval(code, fallback)` runs the fallback on an error. Each counts one level of the nesting budget. `evalc` swaps both sinks for a buffer and returns what was printed less its final line end. `run(script)` runs a file by path or name in the workspace, moving to its folder for the run |
| `now`, `clock`, `datestr`, `pause` | 13 | `environ.rs` `dates_round_trip_through_date_numbers` | Local time through `GetLocalTime` or `localtime_r`; `datestr` is `dd-mmm-yyyy HH:MM:SS`. `pause(n)` sleeps from 0 to 86400 seconds; a bare `pause` waits for Enter at a terminal and is a clean error anywhere else |
| `getenv`, `system`, `version` | 13 | golden | `system(cmd)` runs `cmd /C` or `sh -c` in the current folder with no standard input; one output prints the command's output and returns the status, two return both. `version` is `Cargo.toml`'s version |
| `exit`, `quit`, `exit(n)` (QA D28) | 13 | `interp.rs` `exit_passes_every_frame_and_eval_runs_text`, `tests/cli.rs` | Statements anywhere: a script, a block, a function, `eval`; no `try` catches them; `exit(n)` exits with `n`, a whole number from 0 to 255. Under `--protocol`, `--ui` and `--http-stdio` they are a clean error and the session goes on |
| `clc` | 13 | golden | The terminal's clear only when standard output is a terminal; nothing in a pipe, `evalc`, `--protocol` or `--ui` |
| Terminal line editor | 13 | `editor.rs`, `history.rs` | When standard input and output are both terminals: arrows, Home/End, Backspace/Delete, Up/Down history, Ctrl-C clears, Ctrl-D on an empty line ends, Tab completes through `env::completions`. A pure state machine inside `term.rs`'s raw-mode shell, which restores the terminal on every path. A pipe reads plain lines as before |
| History file | 13 | `history.rs` | `SPLATCRAB_HISTORY`, else `~/.splatcrab_history`: UTF-8, one entry per line, `\\`, `\n` and `\r` escaped, newest 1000 kept |
| `completions` lists path files | 13 | `env.rs` `path_files_are_listed_by_their_function_names` | The `.m` files of the current folder and the `addpath` folders, by function name, for the protocol and Tab alike |
| `--help` and `--version` | 13 | `tests/cli.rs` | Both exit 0; the version comes from `Cargo.toml` through `env!` |

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
| REPL with block and bracket continuation | 00 | `exit` and `quit` leave. Since U0 the continuation test is `syntax::is_complete`, shared with the protocol's `complete`; since 13 `exit` is a statement and the line editor runs at a terminal |
| `.proto` golden cases | U0 | `tests/golden.rs` spawns the binary with `--protocol` and types the case on stdin |
| `.http` golden cases | U1 | `tests/golden.rs` spawns the binary with `--http-stdio --port 8123 --token test-token` and pipes the case on stdin; an `.http` case with no `.err` asserts an empty stderr |
| The UI server over a real socket | U1 | `tests/ui_server.rs` spawns `--ui --port 0 --no-browser --token itest` and talks to it over `TcpStream`, killing it in a `Drop` guard; it also checks the refusals of bad `--ui` and `--http-stdio` options |
| Script runner, exit code 1 on error | 00 | stdout is flushed before the error |
| The Windows console reads output as UTF-8 | 02 | `SetConsoleOutputCP(65001)`, a raw `extern "system"` declaration in `src/main.rs` rather than a crate, so `×` and non-ASCII text render. A pipe or a file is unaffected |
| Golden-file test harness | 00 | `tests/golden.rs`, no dependencies |
| Zero dependencies | 00 | And zero dev-dependencies |
