# 06 — Function handles

## Goal

`@name`, `@(x) body` with capture at creation, `Value::Func`, calling handle variables, `nargout` propagation through single-call bodies, `feval arrayfun func2str str2func`, lexer rule for `@(…)` inside `[]`/`{}`

After this module a user can do things that were not possible before; every
bullet in Scope must be demonstrable by at least one acceptance test below.

## Scope

- `@name`
- `@(x) body` with capture at creation
- `Value::Func`
- calling handle variables
- `nargout` propagation through single-call bodies
- `feval arrayfun func2str str2func`, and `class` and `isa` knowing
  `'function_handle'`. `is_function_handle` is Octave's, not MATLAB's:
  MATLAB's check is `isa(f, 'function_handle')`, which item 7 uses, so it
  is left out for the reason cycle 01c removed Octave's `e`
- `@name` binds when the handle is created, following invariant 4's order
  from where it is created: a handle to a function local to a file keeps
  calling that function when it is called from somewhere the name would
  not resolve
- Every builtin here that calls a function, `feval` of a handle and
  `arrayfun`, goes through `Interp::call_nested`, which counts each call
  against the shared nesting budget. Cycle 05's review found `feval`
  re-entering the interpreter uncounted and overflowing the stack; a
  handle that calls a builtin that calls the handle must stay bounded too
- `func2str` renders an anonymous function from its parse tree, not by
  editing its source text: no spaces around binary operators, and a comma
  between bracket elements, so `@(x) [x 1]` keeps two elements. Only the
  strings this spec records are asserted; the rendering of other forms is
  SplatCrab's own until a source settles it
- The cycle 05 error trace names an anonymous function by its `func2str`
  text. That is SplatCrab's choice, recorded in the Design notes and
  asserted by no case (verify first)
- The protocol's `workspace` lists a handle with class `function_handle`
  and size `[1,1]`; U0's `vars` shape is unchanged
- lexer rule for `@(…)` inside `[]`/`{}`

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.
- `arrayfun(..., 'UniformOutput', false)`, which returns a cell, and
  `cellfun`: cycle 07.
- `is_function_handle`, which MATLAB does not have.


## Design notes

### What changed, file by file

- **`parser.rs`**: `Expr::FuncHandle(String)` for `@name` and
  `Expr::AnonFn(Rc<AnonFn>)` for `@(params) body`. `AnonFn { params, body,
  free }`: `free` is every name the body reads that is not a parameter, in
  first-read order, nested anonymous functions' free names included, computed
  once by `AnonFn::new` at parse time. Parameters are identifiers or `~`, as
  a function's are. The body is one `parse_expr` with `in_index` cleared, so
  it runs to the end of the expression and stops at the comma of an argument
  list, and `end` in it means nothing until it opens an index of its own. An
  `@` followed by neither a name nor `(` is still `unexpected '@' in
  expression`, so `03-indexing-forms/err_at_sign_parse` is unchanged.
  `parse_matrix` refuses a handle as an element. `render` and `AnonFn::text`
  are `func2str`'s rendering; `Parser::parse_handle` reads a `str2func` text.
- **`lexer.rs`**: the `@(…)` rule. The `(` after an `@` is pushed as `P`; its
  `)` pushes an `A` when a `[`, a `case {` or a `{` is directly around, and
  the `,`, `;`, newline, `]`, `}` or `)` that ends the element pops it.
  Under an `A` the whitespace rule is off. A quote straight after a
  parameter list's `)` opens a string, so `@() 'hi'` is not a transpose.
- **`value.rs`**: `Value::Func(Rc<Func>)` and `Func::{Named, Anon}`, with
  `FUNC_CLASS`; `mat` and `into_mat` refuse any non-matrix variant with its
  own class name.
- **`interp.rs`**: `Unit` is public and `Debug`, since a handle holds one.
  `Callee`, `named_handle`, `anon_handle`, `call_global` (split out of
  `call_function`), `call_nested` taking a `Callee`, `call_handle`,
  `call_anon`, `eval_request` (the statement's, the multiple assignment's and
  the body's shared "a call is asked for n values" path), `handle_form`,
  `str2func`. `eval_access` and `apply_access` call a handle on `(...)`;
  `hcat` refuses one.
- **`builtins/core.rs`**: `feval` of a handle, `arrayfun`, `func2str`,
  `str2func`, `isa` of a handle. 99 builtins become 102.
- **`error.rs`**: `handle_concatenation`, `arg_not_a_handle`,
  `arrayfun_size`, `arrayfun_nonscalar`.

### Capture and binding

An anonymous function captures, when the `@(...)` is evaluated, each of its
free names that is a variable of the running frame, with its value; the
snapshot is a clone, so a later assignment to the variable does not reach it.
A free name that is not a variable then is never looked up as a variable
again: in the call's frame only the captures and the parameters are
variables, so the name is a function, resolved against the file the handle
was made in (its `unit`) and then the script, path and builtins. That is why
item 14's `g = @(n) g(n)` is the undefined-name error, and why a handle
passed to a function cannot see that function's variables. `str2func`'s
anonymous functions capture nothing.

`@name` looks the name up among the local functions (the running file's,
then the script's) where it is made, by invariant 4's order, and keeps what
it finds with its file. With nothing local, the handle holds the name alone
and resolves it on the path and among the builtins when called, never among
the local functions of wherever it has been passed to. A variable of the name
plays no part. Binding a path file at creation was rejected: the path can
change between creation and call, and a lookup at call time is what `feval`
of a name already does.

### Calls, frames and `nargout`

A call of an anonymous function refuses more arguments than parameters
before anything runs (no trace entry), then counts against `MAX_RECURSION`
and pushes a `Frame` with the captures, then the parameters, an empty
`end_stack`, no `func_name` and the handle's `unit`. The body runs through
`eval_request`: a body that is a call by name, or a call of a handle
variable with one `(...)`, is asked for the caller's `nargout`, so
`[m, i] = f(v)` with `f = @(v) max(v)` asks `max` for two, and a statement's
`f()` asks for none, so `@() disp(1)` is legal as a statement and sets no
`ans`. Any other body is its one value; asked for two, the caller's check
makes it `Too many output arguments.`

Counting anonymous calls against the recursion limit is SplatCrab's choice:
it keeps the stack bounded when recursion runs through an anonymous function
(`r = f(n)` calling `g = @(k) f(k + 1)`), at the cost of halving the user
recursion depth such a pattern reaches. MATLAB's accounting is unverified.

Every builtin that calls a function goes through `Interp::call_nested`, which
now takes a `Callee`, a name or a handle, and counts one level of the shared
nesting budget: `feval` of either, and each element call of `arrayfun`.
`feval`'s peel of a leading run of `'feval'` names now also peels unbound
`@feval` handles, so `feval(@feval, 'feval', ..., @sin, 0)` is one call.

### Choices where the spec was silent

- **`disp` of a handle** prints the handle as its display shows it, without
  the indent: `@(x)x+1` for an anonymous function, which is its `func2str`
  text, and `@sin` for a named one, whose `func2str` is `sin`.
- **The display** is `f =`, a blank line, `  function_handle with value:`, a
  blank line, `    @(x)x+1` and the closing blank line every named display
  has; `@sin` for a named handle.
- **The trace entry** of an anonymous function is its `func2str` text with no
  line, `  in @(n)g(n)`: an expression carries no line of its own. No case
  asserts it (verify first).
- **`func2str` rendering** beyond the recorded strings: parentheses only
  where precedence needs them (`(x+1)*2`, `-(x+1)`, `(-x)^2`, but `2^-x` and
  `-x^2`); a stepped range on the left of a colon unbracketed (`1:2:3:4`), a
  plain one bracketed (`(1:2):3`); `;` between bracket rows; strings in single
  quotes with `''`; numbers in the shortest round-tripping form, with an
  exponent outside `1e-5 <= |v| < 1e15` (`1e-20`, `1e+20`); `x'` for both
  transposes. A rendered body parses back to the same tree (unit test).
- **`arrayfun`**: the element calls are asked for `arrayfun`'s own `nargout`
  (so `[m, k] = arrayfun(@(x) max([x 5]), v)` works). As a statement it asks
  for none, and if the first call gives a value anyway, every call must, and
  the one output is built; if it gives none, the calls are made for their
  effect and there is no output. An empty input gives an empty of its shape.
  The output's class is the results' when they all share one, else double.
  A non-handle first argument is `Argument 1 to 'arrayfun' must be a function
  handle.` Arguments after the arrays are arrays too, so `'UniformOutput'`
  meets the size check; the option is cycle 07's.
- **`str2func`** of a text that does not start with `@` makes a named handle
  of the trimmed text without checking it is a name, so a bad name fails
  when called, as a handle to a missing function does. A text that fails to
  parse gives the parser's message with no line of its own, and the calling
  statement's line; `'@sin + 1'` is `unexpected '+'`.
- **`isa(f, name)`** of a handle is true for `'function_handle'` alone, in
  no group. `class(f)` is `function_handle`.
- **Concatenation**: `[@(x) x+1]` and `[1 @sin]` are refused by the parser,
  `[f 1]` and `[f]` at run time by `hcat`, both with `Nonscalar arrays of
  function handles are not allowed; use cell arrays instead.`, MATLAB's text
  for concatenating handles as recalled, not confirmed.
- **Chained calls**: `add(3)(4)` calls the handle `add(3)` returns, as
  chained indexing reads each link in turn (an Octave-style deviation already
  recorded for indexing).
- **Message texts**, all from recall or SplatCrab's own, none confirmed:
  `All of the input arguments must be of the same size and shape.` and
  `Non-scalar in Uniform output, at index N, output M. Set 'UniformOutput'
  to false.` (MATLAB's as recalled, the second on one line), and
  `Argument N to 'f' must be a function handle.` (SplatCrab's, for
  `arrayfun` and `func2str`).

### Bytes settled in testing

The tests were written before the code ran, and these lines the Acceptance
tests do not record were settled against the rules above:

- **The three self-application cases** (`err_anonymous_self_application`,
  `err_anonymous_feval_self_application`,
  `err_anonymous_arrayfun_self_application`) assert the full `Maximum
  recursion limit of 500 reached.`: every anonymous call counts against
  `MAX_RECURSION` (Calls, above) and each level adds one or two such calls
  and only a few nesting levels, so the limit of 500 is always met before
  the nesting limit of 10,000.
- **`err_feval_handle_recursion_limit` and `err_arrayfun_recursion_limit`**
  keep the full recursion message: the user function `viaf`/`viaa` is one
  counted frame per level, and cycle 05's `err_feval_recursion_limit` is the
  precedent.
- **Error texts the spec does not record are SplatCrab's own**, pinned in
  full in their cases and not MATLAB-sourced: `All of the input arguments
  must be of the same size and shape.` (`err_arrayfun_size_mismatch`),
  `Non-scalar in Uniform output, at index 1, output 1. Set 'UniformOutput'
  to false.` (`err_arrayfun_nonscalar_result`), `Nonscalar arrays of
  function handles are not allowed; use cell arrays instead.`
  (`err_handle_in_brackets` from the parser, `err_handle_concat_at_run_time`
  from `hcat`), and `Argument 1 to 'arrayfun' must be a function handle.`
  and `Argument 1 to 'func2str' must be a function handle.`
  (`err_arrayfun_not_a_handle`, `err_func2str_not_a_handle`). The first
  three follow MATLAB's wording as recalled; a MathWorks source that
  differs wins.
- **Every `Error: Line N:`** names the script's own statement, cycle 05's
  rule; where a handle is made and called on one line, that line.
- **No trace line is asserted.** The anonymous entries, `  in @(g,n)g(g,n+1)`
  and the like, are SplatCrab's (The trace entry, above), and a recursion
  trace is 500 lines.
- **`handle_display.out`** ends at `    @(x)x+1`: the closing blank line is
  printed (the protocol's `out` ends in two newlines) but the harness drops
  trailing blank lines.

Crash probes run at this point (in testing), all exit 0 or 1 with no stderr under
`--protocol`: a 10,000-link chain of handles each capturing the last
(`h = @(n) h(n) + 1` in a loop; the recursion limit), an anonymous function
handed itself directly, through `feval` and through `arrayfun` (the
recursion limit), `arrayfun` nested 10,000 deep in the source (the nesting
limit at parse) and 400 deep (runs), `feval(@feval, @feval, ..., @sin, 0)`
10,000 long (peeled, runs), a 1e6-element capture made and copied 1,000
times (2 s), `str2func` of a 100,000-deep text (the nesting limit), of
`'#$%^&'` (fails when called), `'@(x'`, `'@'`, `''` and a trailing `;`,
`func2str` of bodies 9,000 deep in parentheses, unary minus and nested
`@()`, from the source and from `str2func`, 10,000 parameters, a handle to
a path function whose file is deleted, rewritten and removed from the path
between calls (`Unrecognized function or variable 'pf'.`, then the new
file's value, then the error again), `@end`, `@1`, `@(1) x`, `@(x)`, `@() :`,
`@() end`, and `[a, b, c] = f(v)` with `f = @(v) max(v)` (`Too many output
arguments.`). `@(x, x) x` is accepted, as `function r = f(x, x)` already is.

### Invariants

1. Column-major storage: `arrayfun` visits elements and builds its outputs in
   storage order.
2. The one-based to zero-based conversion stays in `eval_index_args`.
3. `end`: an anonymous call's frame has its own `end_stack`, so `x(f(end))`
   binds `end` to `x` and `@() w(end)` binds it to the captured `w`
   (`an_anonymous_call_has_a_frame_of_its_own`).
4. Resolution: unchanged for names; `@name` applies it at creation, and an
   unbound handle skips the local functions at call time
   (`a_named_handle_keeps_its_binding`).
5. Output: unchanged; the trace is still printed by `main.rs`.
6. Every call back from a builtin is counted (`call_nested_counts_against_the_nesting_budget`),
   and every recursion through a handle meets the limit
   (`recursion_through_handles_is_bounded`).

### The stack

Measured as cycle 05 did, with a copy of the tree whose `main.rs` read the
thread size from the environment, bisecting the smallest stack in MB that
exits 0 or 1 in a debug build:

| Script | cycle 05 HEAD | this cycle |
|---|---|---|
| 9,990 levels of `abs(` | 172 | 172 |
| 9,990 levels of `[` | 153 | 155 |
| 500 frames of `r = g(n)` through `g = @(k) f(k + 1)` | | 5 |
| the same, each with 18 levels of `abs(` around the call | | 44 |
| frames to 400, then a body of 9,000 levels of `abs(` | | 155 |
| a `str2func` text 5,000 deep, called from 5,000 levels of `abs(` | | 130 |
| recursion through `arrayfun`, through `feval` of an anonymous function | | 6, 4 |

So the worst case is still the 10,000-level builtin chain at 172 MB of the
256, and nothing new approaches it.

**Fixed at review.** The probes above were too small to see two
defects the review found, both in code new this cycle, and both breaking
invariant 6:

- A chain of handles, each capturing the one before
  (`h = @() 1; for k = 1:500000, h = @() h() + 1; end; clear h`), was freed
  by the default recursive drop, one stack frame per link, and overflowed
  the stack at about half a million links (exit 134 on Linux), at `clear`
  or when the workspace was freed at exit, and in a `--protocol` session.
  `Func` now has a `Drop` that frees captured handles from a worklist,
  taking each out with `Rc::try_unwrap`, so no drop recurses more than one
  level. A value that later holds handles, cycle 07's cell array, must feed
  the same worklist. The unit test
  `a_long_chain_of_captured_handles_is_freed_without_recursion` overflows
  on Rust's 2 MB test thread without the fix and passes with it, which was
  checked; `handle_chain_freed` is the reviewer's script.
- The names a body reads were stored on every `AnonFn` as a list built
  from the lists of the functions nested in it, with repeats found by
  rescanning the list: 60,000 names took 25 s, and 3,000 nested `@()`
  around 3,000 names took 174 s. `AnonFn::free_names` is now one walk of
  the tree per creation, linear in its size, with bound parameters counted
  in a `HashMap` and repeats found through a `HashSet`, and nothing is
  stored. Those two scripts now take 0.29 s and 0.13 s; `free_names_nested`
  is the second, whose old cost is past the harness's timeout.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/06-function-handles/`. Expected output is written by hand from MATLAB
semantics wherever feasible; the `.err` substring is always chosen by hand.
Every new error message needs an `err_*` case.

1. `f = @(x) x.^2; disp(f(4)); g = @(x, y) x + y; disp(g(1, 2)); z = @() 42; disp(z())` → `    16\n     3\n    42`. Cases: anonymous_call.
2. `a = 10; f = @(x) x + a; a = 0; disp(f(1))` → `    11`. Cases: capture_at_creation.
3. `g = @sin; disp(g(0)); h = @sq; disp(h(3))\nfunction r = sq(x)\nr = x * x;\nend` → `     0\n     9`. Cases: named_handles.
4. `disp(arrayfun(@(x) x * 2, [1 2 3])); disp(arrayfun(@(a, b) a * b, [1 2], [3 4]))` → `     2     4     6\n     3     8`. Cases: arrayfun_uniform, err_arrayfun_size_mismatch, err_arrayfun_nonscalar_result, err_arrayfun_not_a_handle.
5. `disp(feval(@(x) x + 1, 1)); disp(feval('sin', 0))` → `     2\n     0`. Cases: feval_handle_and_name.
6. `disp(func2str(@(x) x.^2 + 1)); f = str2func('@(x) x*3'); disp(f(2)); disp(func2str(@sin))` → `@(x)x.^2+1\n     6\nsin`. Cases: func2str_str2func, func2str_brackets_round_trip, err_func2str_not_a_handle.
7. `f = @(x) x + 1` → `f =\n\n  function_handle with value:\n\n    @(x)x+1\n`; `disp(class(f)); disp(isa(f, 'function_handle'))` → `function_handle\n   1` (`isa` returns a logical, four wide since cycle 02). Cases: handle_display, handle_class_isa, workspace_lists_handle.
8. `f = @(v) max(v); [m, i] = f([1 5 2]); disp(i)` → `     2`. Cases: nargout_through_handle.
9. `add = @(a) @(b) a + b; add3 = add(3); disp(add3(4))` → `     7`. Cases: nested_anonymous.
10. `f = @(x) x; f(1, 2)` → err `Too many input arguments.`. Cases: err_handle_too_many_inputs.
11. `f = @(x) x + 1; f(2)` → `ans =\n\n     3\n`. Cases: handle_statement_ans.
12. Lexer and parser unit tests only, since cells are cycle 07 and nothing here evaluates one: `{@(x) x + 1, 2}` tokenizes and parses as two elements; `[@(x) x+1]` is a parse error. Cases: err_handle_in_brackets, err_handle_concat_at_run_time (golden cases for the refusal's message, which every new message needs; the lexer half stays unit tests).
13. `r = apply(@(x) x * 2, 5); disp(r)\nfunction r = apply(f, v)\nr = f(v);\nend` → `    10`, and a handle to a local function called from a path file's function → the local function's result: a handle keeps the binding it was created with. Cases: handle_passed_to_function, handle_local_from_path_file, handle_to_subfunction_returned.
14. `g = @(n) g(n); g(1)` → err `Unrecognized function or variable 'g'.`: capture happens at creation, when `g` does not exist yet, so inside the body `g` is looked up as a function and there is none. And `r = viah(1)\nfunction r = viah(n)\nh = @viah;\nr = h(n + 1);\nend` → err `Maximum recursion limit of 500 reached.`, exit 1: recursion through a handle meets the same limit as a direct call. Cases: err_capture_before_exists, err_handle_recursion_limit, err_feval_handle_recursion_limit, err_arrayfun_recursion_limit, err_anonymous_self_application, err_anonymous_feval_self_application, err_anonymous_arrayfun_self_application.

## Status

Done (2026-09-28)
