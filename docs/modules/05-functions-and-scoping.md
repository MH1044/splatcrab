# 05 — Functions and scoping

## Goal

`function` blocks in scripts and `.m` function files on a path, `Frame` stack (`end_stack` moves into the frame), resolution variable → local fn → script fn → path file → builtin, `nargin/nargout`, `return` (`Flow::Return`), recursion limit 500, subfunctions, scripts on the path run in the caller's workspace, file cache with generation counter, `exist feval addpath rmpath`

After this module a user can do things that were not possible before; every
bullet in Scope must be demonstrable by at least one acceptance test below.

## Scope

- `function` blocks in scripts and `.m` function files on a path
- `Frame` stack (`end_stack` moves into the frame)
- resolution variable → local fn → script fn → path file → builtin
- `nargin/nargout`
- `return` (`Flow::Return`)
- recursion limit 500
- subfunctions
- scripts on the path run in the caller's workspace
- file cache with generation counter
- `exist feval addpath rmpath`. `exist` returns what the MathWorks `exist`
  page lists: `1` for a variable, `2` for a file on the path, `5` for a
  built-in function, `0` otherwise. Every SplatCrab builtin is built in
  here, so each is `5`, where MATLAB gives `2` for the ones it ships as
  `.m` files (`linspace`, for instance); that is a recorded deviation. The
  page does not say what `exist` reports for a function local to the
  running script, so no case asserts it (verify first)
- **`Interp::cwd`**, seeded from the process's working directory, and the
  directory every path lookup of this cycle resolves against: function
  files, `addpath` and `rmpath`. This cycle is the first to resolve paths,
  so it introduces the field that cycle 13's spec makes interpreter state;
  13 adds `cd` and `pwd` on top of it. Resolving against `std::env` here
  would have to be revisited in every path builtin later
- **The error trace.** An error raised inside a user function and not
  caught prints its message and then one `  in <fn> (line N)` line per
  frame, innermost first, as the Errors key design in
  `docs/ARCHITECTURE.md` has always planned. `e.stack` itself is not
  this cycle's: MATLAB's is a struct array, and structs arrive in cycle
  07, which takes it (cycle 04 had deferred it here)
- **Where a function may be defined.** In a script, local functions come
  after all the script's statements; a statement after one is MATLAB's
  `Function definitions in a script must appear at the end of the file.`
  At the REPL, in a protocol `eval` and in the browser page, a `function`
  block is refused with MATLAB's `Function definitions are not supported
  in this context.` Both texts are MATLAB's, confirmed by the titles of
  MathWorks Answers threads; only these first sentences are asserted
- Under `--protocol` and `--ui`, an error raised inside a function keeps
  U0's meaning of `line`: the line within the submitted code of the
  statement that failed, the outermost frame, never a line of another file.
  The trace is not added to the protocol's error object here; cycle U3's
  editor, which jumps to an error's line, is where it is needed
- `syntax::is_complete` counts `function ... end` as a block, so the REPL
  and the page read a whole definition before refusing it, rather than
  running half of one
- The recursion limit holds invariant 6: 500 frames, each evaluating a
  deeply nested expression, fit the 256 MB interpreter thread, and 501
  is the clean recursion error, never an abort

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.
- `global` and `persistent` unless they fall out cheaply once frames exist.
- Class definitions and packages.

## Design notes

Filled in as decisions are made. Start from the
"Key designs to preserve" section of docs/ARCHITECTURE.md and record here:
which files and types changed, which invariants were preserved (column-major
storage, 1-based to 0-based conversion at the index boundary, the `end` stack,
name resolution order, the output sink), and any deviation from MATLAB that
was accepted deliberately.

### What changed, file by file

- `src/lexer.rs`: the keywords `Function` and `Return`, per the "Add a
  statement" recipe. A `function` line's outputs and parameters (every name
  on it but the function's own) become variables of the body for the
  command-syntax rule, so `a -1` is an expression inside `function r =
  f(a)`. Each `function` also forgets the names assigned before it and stops
  consulting the workspace the source was lexed against, since a function's
  workspace is its own.
- `src/parser.rs`: `Program { stmts, functions }`, `Function { name,
  outputs, params, body, line }`, `Stmt::Return`, and `Parser::at_depth`.
  `parse_program` returns a `Program`: statements, then functions; a
  statement after a function is `functions_at_end` on its own line, and a
  `function` inside a block is `function_not_supported_here`.
- `src/syntax.rs`: `is_complete` counts `function` as an opener.
- `src/interp.rs`: `Frame { vars, end_stack, unit, func_name, nargin,
  nargout }`, `Unit` (one parsed text: a script's statements and local
  functions, or a function file's entry and subfunctions), `CachedFile`,
  `MAX_RECURSION`, `Flow::Return`, and on `Interp` the fields `frames`,
  `script`, `calls`, `cwd`, `search_path`, `generation`, `files` and
  `lookups`. `Interp.vars` became `Interp::vars()` and `vars_mut()`, the
  running frame's. New: `run_command`, `call_function`, `local_function`,
  `call_user`, `run_script`, `find_file`, `load`, `call_counts`, `exist`,
  `add_path`, `remove_path`. `call_builtin` is now reached only through
  `call_function`.
- `src/error.rs`: `StackEntry`, `MError::leaving`, `trace`, `stack()` and
  `identifier()`; the identifier and the stack are boxed together in a
  private `extra` (see "The stack" below). Texts: `functions_at_end`,
  `function_not_supported_here`, `output_not_assigned`, `recursion_limit`,
  `nargin_outside_function`, `cannot_read`, and the two warnings
  `addpath_not_a_folder` and `rmpath_not_on_path`.
- `src/builtins/core.rs`: `nargin`, `nargout`, `exist`, `feval`, `addpath`,
  `rmpath` (the registry holds 99). `clear` and `who` act on the running
  frame.
- `src/main.rs`: the REPL runs entries with `run_command`; script mode and
  the REPL print `MError::trace` after the message. `src/protocol.rs`:
  `eval` runs `run_command`, so `--ui` refuses a definition too.

### Where a function may be defined, and `end`

- A text whose statements are empty and which defines a function is a
  **function file**: its first function is its entry, the one its file name
  calls, whatever name its header gives; the rest are subfunctions, found by
  their own names from inside that file only. Anything else is a script,
  whose functions are local functions. A second definition of a name in one
  file is ignored (the first wins); MATLAB refuses it.
- **The `end` rule implemented**: a body is read as a block that stops at
  `end` or at the next `function`. The blocks inside a body consume their own
  `end`s, so the first `end` at the body's level is the function's; a body
  that stops at `function` or at the end of the text had none. So a function
  file whose functions all go without `end`, MATLAB's older form, reads as
  MATLAB reads it. SplatCrab does not enforce MATLAB's "all or none": a file
  that mixes the two is accepted, and so is a script whose local functions
  have no `end`, where the body then runs to the end of the file, so the
  script-end rule cannot fire. Neither changes what a file MATLAB accepts
  means. No case asserts either.
- A file given on the command line is always run as a script. One that holds
  only functions therefore runs nothing; its functions exist for the run.
- At the REPL, in a protocol `eval` and so in the browser page, the whole
  entry is parsed first and a definition anywhere in it is refused on the
  line of its `function`, before any statement runs.

### Calls

- **Invariant 4** is `Interp::call_function`: after the variable its callers
  rule out, the running frame's `unit` (the running file), then the unit
  `run` is running (the script), then `find_file`, then the registry. It is
  taken literally: a function in a file on the path can call a local function
  of the script being run, which MATLAB, whose local functions are private to
  their file, does not allow (a recorded deviation). A path script, which
  runs in its caller's frame, swaps its own unit in for as long as it runs,
  so its local functions are the running file's.
- `find_file` looks for `name.m` in `Interp::cwd`, then in each `addpath`
  folder in order, so the current folder wins over the path, as in MATLAB's
  precedence list. A name that is not an identifier is never looked up, so
  `feval('../x')` or `exist('a/b')` cannot reach outside.
- `call_user` checks, before anything runs and so with no trace line: more
  arguments than parameters (`Too many input arguments.`), more outputs asked
  for than the function has (`Too many output arguments.`, also for `x = g()`
  where `g` has none), and the recursion limit. It then pushes a `Frame` with
  the parameters bound (`~` binds nothing but counts), clears `loop_depth`
  for the body so a `break` cannot reach the caller's loop, runs the body,
  and pops the frame however the body ended. Asked for `nargout` values it
  returns that many outputs, and the first one asked for and unassigned is
  `output_not_assigned`; asked for none, as a statement asks, it returns the
  first output if it was assigned, which becomes `ans`, and nothing if not.
  An argument left off is simply unbound, so using it is `Unrecognized
  function or variable`, where MATLAB says `Not enough input arguments.`
- `nargin` and `nargout` are builtins reading the running frame; a path
  script sees its caller's. Outside every function they are MATLAB's `You
  can only call nargin/nargout from within a MATLAB function.` `nargin('f')`
  is not supported.
- A path **script** takes no arguments and gives no outputs: `setup(1)` is
  `Too many input arguments.` and `x = setup` is `Too many output
  arguments.`, reusing the two texts rather than adding MATLAB's `Attempt to
  execute SCRIPT setup as a function`, which names the script (a recorded
  deviation). It counts against the recursion limit like a call, since a
  script that runs itself would otherwise recurse without bound.
- `return` is `Flow::Return`: loops pass it on, `call_user` and `run_script`
  end on it, and at the top it ends the script (or the REPL entry).
- `feval('name', ...)` resolves as a call does but skips variables, and
  passes its own `nargout` on. `exist(name)` is `1` for a variable of the
  running frame, `2` for a file `find_file` finds (function or script), `5`
  for a builtin, `0` otherwise; it takes one argument, and a function local
  to the running file is none of those, so it gives `0` (unverified, and not
  asserted). A `name.m` argument is not recognised.
- `addpath(d, ...)` resolves each folder against `Interp::cwd`, puts them at
  the front of the path in the order given, and moves one already on the
  path rather than listing it twice. A folder that does not exist is the
  warning `Name is nonexistent or not a directory: <d>` and is left off;
  `rmpath` of a folder not on the path is the warning `"<d>" not found in
  path.` Both texts are MATLAB's as recalled, not confirmed; the
  `err_path_folder_warnings` case pins them as SplatCrab's own texts, since
  every new message has a case. Neither returns the old path, and `-end`/`-begin` are not
  supported.

### The file cache

`files` maps a path to its parsed `Unit`, the generation it was last known
good in, and the file's modification time and length when it was read;
`lookups` maps a name to the file it resolved to (or none) and the generation
of that answer. `generation` is bumped by `addpath`, by `rmpath` and by every
`run` or `run_command`. An answer from another generation is never used
as it stands: a lookup is redone, and a cached file is reused only if its
modification time and length are what they were, otherwise it is read and
parsed again. **So file modification is detected, between entries**: an edit
is seen by the next REPL line, protocol `eval` or browser entry, and after
`addpath` or `rmpath`, but not in the middle of one run, which is what keeps a
builtin called in a loop from asking the file system every time. A file that
fails to parse is not cached. Each script case of the golden harness is a
fresh process, so none depends on this.

### The error trace and the protocol's line

An error leaving a function or a path script passes `MError::leaving(name)`:
its line, which is a line of that function's file, moves into a new outermost
`StackEntry`, and `line` is cleared, so the calling statement's `MError::at`
records its own line. At the top, `line` is therefore always a line of the
code that was run: script mode's `Error: Line N:` names the script's own
statement, and the protocol's `line` keeps U0's meaning with no change to
`protocol.rs`. `MError::trace` renders the stack, one `  in <fn> (line N)`
per entry, innermost first; `main.rs` prints it to stderr after the message
in script mode and at the REPL. The name is the one the call used: the file
name for a path file's entry, the function's own for a local function or a
subfunction (MATLAB writes `script>g3`; the spec fixes `g3`). A parse error in
a path file, found when it is first called, gains an entry for that file at
the parse error's line. A caught error keeps its stack; `rethrow` raises it
with the stack it had. The checks `call_user` makes before the body runs, and
the output check after it, are raised at the call and carry no entry of their
own.

**The stack.** An `MError` rides in every `R<Value>` the evaluator returns,
and in a debug build every such temporary has its own slot in every frame of
the recursion. Adding the stack as a bare `Vec` grew `MError` from 56 to 80
bytes and cost the 10,000-level `abs(abs(...))` chain 240 MB of stack in a
debug build where cycle 04 needed 198 MB, too close to the 256 MB thread.
Boxing the identifier and the stack together behind `identifier()` and
`stack()` made `MError` 40 bytes and `R<Value>` 48, and the chain 172 MB.
Both are read through accessors now; `e.identifier` in MATLAB is unchanged.

### The recursion limit and invariant 6

`MAX_RECURSION` is 500 and `calls` counts the user calls running, functions
and path scripts alike: 500 are allowed, and the 501st is `Maximum recursion
limit of 500 reached.` (a unit test asserts both sides; no golden case pins
the boundary, per item 17). Every frame shares the evaluator's one nesting
counter, which calls never reset: a budget of `MAX_DEPTH` levels for the
whole thread, however they are spread over frames. A function file parsed at
its first call is parsed on the evaluator's stack, so `Parser::at_depth`
starts its count at the evaluator's depth; without it, a file nested 10,000
deep, parsed from inside 10,000 levels of calls, would need the parser's
145 MB on top of the evaluator's 172 (`a_file_parsed_deep_shares_the_nesting_budget`).

Measured with a copy of the tree whose `main.rs` read the thread size from
the environment, bisecting the smallest stack in MB that exits 0 or 1 (all
of these exit cleanly on the real 256 MB thread):

| Script | debug | release |
|---|---|---|
| 500 frames of `r = f(n + 1)`, to the recursion limit | 8 | 2 |
| 500 frames, each 15 levels of `abs(` around the call | 68 | 14 |
| 500 frames, each 15 levels of `1+(` around the call | 98 | 12 |
| frames each 25 to 1,000 levels deep (`abs(`, `1+(`, `-(`, `[`), which spend the shared budget and end in the nesting error | 120 at most | 16 at most |
| 1 to 500 frames, then 9,990 levels of `abs(` at the bottom | 172 | 44 |
| 1 to 500 frames, then 9,990 levels of `[` at the bottom | 153 | 37 |
| For comparison, cycle 04's HEAD: 9,990 levels of `abs(` in a script | 198 | 46 |
| For comparison, cycle 04's HEAD: 9,990 levels of `[` in a script | 175 | 38 |
| 9,990 nested parentheses, the parser alone | 145 | 37 |

**So 500 frames each evaluating a deep expression fit**: the worst case is the
same 10,000-level builtin chain a single frame could already build, at 172 MB
of the 256 in a debug build, which is what the golden harness runs, and 44 in
release. Nothing was lowered. Two things the measurement found are recorded
for later rather than fixed here: the 01e Design notes' "about 28x" margin on
the evaluator was measured on a flat sum and does not hold for nested calls,
which cost about 17 KB a level in debug; and the margin that is left in a
debug build is about 1.5x, so a change that grows `eval_node`, `eval_access`
or `R<Value>` should be measured the same way.

### Invariants

1. Column-major storage: untouched.
2. The one-based to zero-based conversion stays in `eval_index_args` alone.
3. `end` is resolved through the running frame's `end_stack`
   (`end_inside_call`, `a_frame_isolates_end`).
4. Resolution is `call_function`, in the order above (`names_resolve_in_order`,
   `variable_shadows_function`, `local_shadows_path_file`, `addpath_shadow`).
5. All output still goes through `emit`; the warnings through `emit_err`; the
   trace is printed by `main.rs`, the only file allowed to.
6. Deep recursion is a clean error at 501 calls, and the stack table above.

### Deviations accepted deliberately

- `exist` gives `5` for every builtin, `2` in MATLAB for its `.m` builtins;
  `0` for a local function, unverified.
- The trace names `g3`, not `script>g3`.
- A path function can call the running script's local functions; a file may
  mix `end` forms; a script's local functions may omit `end`; a duplicate
  definition is ignored rather than refused.
- A script called with arguments or for a value reuses the argument and
  output texts.
- An argument left off and then used is the undefined-name error, not
  `Not enough input arguments.`

### Bytes the cases settle

The test pass checked each line of the cases whose bytes the Acceptance
items leave open against the rules above, and pinned those the rules fix:

- **The `Error: Line N:` prefix of an error that left a function** names the
  script's own statement, per "The error trace" above:
  `err_caller_variable_invisible` (`Line 7`, the `g3()` call) and
  `err_trace_two_frames` (`Line 5`). The trace lines follow the message line
  directly, one per frame, innermost first, and an outer frame's line is the
  line of the call it was running (`in outer_fn (line 7)`).
- **Errors `call_user` raises at the call** name the calling line and carry
  no entry of their own: `err_too_many_inputs` (`Line 4`),
  `err_output_not_assigned` (`Line 3`), and `err_recursion_limit` (`Line 4`,
  then `  in inf_rec (line 6)` for each of the 500 frames that were running;
  only the first is asserted). That `sq(1, 2)` prints no stack line cannot be
  asserted by a substring; the unit test `the_trace_lists_each_frame_left_innermost_first`
  and the rule above are what hold it.
- **`functions_at_end`** names the misplaced statement's own line:
  `err_function_before_statement` (`Line 6`), and nothing before it runs.
- **The refusal's protocol `line`** is the line of the `function` keyword
  within the submitted code, `1` in `err_eval_function_refused`, per "Where
  a function may be defined" above; the message is the first sentence alone.
- **The REPL's continuation prompt** is three spaces (`src/main.rs`), so a
  two-line entry shows `>>    >>` in `err_repl_function_refused`.
- **`err_recursion_nested_expression`** ends in the recursion limit, not the
  nesting limit, because a parenthesis costs no evaluator level; the case
  still asserts only `Error: Line` and exit 1, since the spec leaves which
  limit trips first open.

### Left open

- `tests/cases/00-baseline/who.m` now fails: the case file is itself named
  `who.m` and sits in the current folder of its own run, so its `who`
  statement resolves to the case file (invariant 4, as MATLAB's precedence
  puts the current folder before a builtin) and recurses to the limit. The
  case needs a name that is not a builtin's; that is a separate
  decision, since it is outside this cycle's directory. Run under the name
  `who_listing.m`, the same script prints exactly `who.out` and exits 0.

**Fixed at review.** Two defects the review found, and one platform
difference:

- `feval` re-entered the interpreter with nothing counting: 499 frames each
  running a 300-long `feval('feval', ..., 'f', n + 1)` chain overflowed the
  stack (exit 134 on Linux), and a long run of `'feval'` names copied the rest
  of its arguments once per name. Now a run of leading `'feval'` names that
  reach the builtin is peeled off in a loop, and the call itself goes through
  `Interp::call_nested`, which counts one level of the shared nesting budget.
  Any builtin that later calls back into the interpreter (`arrayfun`,
  `cellfun`) must go through the same function. `feval_chain_collapses` is
  the reviewer's script at a size checked to crash the pre-fix build;
  `err_feval_recursion_limit` shows recursion through `feval` meeting the
  same limit as a direct call.
- `00-baseline/who.m` began calling itself, because a file in the current
  folder now shadows a builtin. The code is right; the case was renamed
  `who_listing`.
- File lookup is case-sensitive on every platform. Windows and macOS file
  systems ignore case, so `ADDONE(1)` found `addone.m` there and not on
  Linux; `find_file` now also requires the directory to list the exact name
  (`err_case_sensitive_lookup`). MATLAB on such a platform is understood to
  name the closest match in its message, which is not reproduced (verify
  first).
- The nargin/nargout message and the two path warnings are SplatCrab's
  wording from recall, not a confirmed MATLAB source; their cases pin them
  as SplatCrab's own.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/05-functions-and-scoping/`. Expected output is written by hand from MATLAB
semantics wherever feasible; the `.err` substring is always chosen by hand.
Every new error message needs an `err_*` case.

1. `disp(sq(4))\nfunction y = sq(x)\n    y = x^2;\nend` → `    16`. Cases: local_function_call, end_inside_call.
2. `[s, p] = sp(2, 3)\nfunction [s, p] = sp(a, b)\ns = a + b; p = a * b;\nend` → `s =\n\n     5\n\np =\n\n     6\n`. Cases: multiple_outputs.
3. `disp(f(1)); disp(f(1, 2))\nfunction r = f(a, b)\nif nargin < 2, b = 10; end\nr = a + b;\nend` → `    11\n     3`. Cases: nargin_default, err_nargin_outside_function.
4. `disp(h()); h\nfunction r = h()\nr = nargout;\nend` → `     1` then `ans =\n\n     0\n`. Cases: nargout_values.
5. `fprintf('%d\n', fact(10))\nfunction r = fact(n)\nif n <= 1, r = 1; else, r = n * fact(n - 1); end\nend` → `3628800`. Cases: fact_recursion.
6. `x = 1; g2(); disp(x); g3()\nfunction g2()\nx = 99;\nend\nfunction g3()\ndisp(x)\nend` → `     1` then err `Unrecognized function or variable 'x'.` (the text since cycle 01e) with stack line `  in g3 (line 8)`, counting the case's `% covers:` line as line 1 only if the case keeps this layout; derive N from the file actually written. Cases: err_caller_variable_invisible, err_trace_two_frames.
7. `disp(early(5)); disp(early(-5))\nfunction r = early(x)\nr = 0; if x > 0, r = 1; return; end\nr = -1;\nend` → `     1\n    -1`. Cases: early_return.
8. Helper `addone.m` beside `path_test.m` containing `disp(addone(41))` → `    42`. Cases: path_test.
9. Helper `helper.m` with subfunction `twice`; `disp(helper(3)); twice(3)` → `     6` then err `Unrecognized function or variable 'twice'.`. Cases: err_subfunction_private.
10. Helper script `setup.m` (`a = 7;`); `setup; disp(a)` → `     7`. Cases: script_on_path, script_in_function_workspace.
11. `sq(1, 2)` → err `Too many input arguments.` (no stack line); `z = bad(1)\nfunction y = bad(x)\nend` → err `Output argument "y" (and maybe others) not assigned during call to "bad".`. Cases: err_too_many_inputs, err_output_not_assigned.
12. `inf_rec(1)\nfunction r = inf_rec(n)\nr = inf_rec(n + 1);\nend` → err `Maximum recursion limit of 500 reached.` without crashing. Cases: err_recursion_limit, err_recursion_nested_expression.
13. `sq(3)` at statement level → `ans =\n\n     9\n`; `disp(feval('sq', 3))` → `     9`; `x = 1; disp(exist('x')); disp(exist('max')); disp(exist('nosuch'))` → `     1\n     5\n     0`, and `exist('addone')` for item 8's path file → `     2`; `exist('sq')` for a function local to the script is not asserted (verify first). Cases: statement_call_ans, feval_local_function, exist_kinds, exist_path_file.
14. Helper `shadow/max.m` returning 42; `addpath('shadow'); disp(max([1 5 2])); rmpath('shadow'); disp(max([1 5 2]))` → `    42\n     5`. Cases: addpath_shadow, addpath_after_lookup, err_path_folder_warnings, variable_shadows_function, local_shadows_path_file, subfunction_before_script_function.
15. `x = 1;\nfunction f()\nend\ny = 2;` → err `Function definitions in a script must appear at the end of the file.`; at the REPL (a `.repl` case) and under `--protocol`, `function f()\nend` → err `Function definitions are not supported in this context.`, the protocol answering it as an eval error and exiting 0. Cases: err_function_before_statement, err_repl_function_refused, err_eval_function_refused, err_eval_error_line_outermost.
16. Under `--protocol`, `complete` on `function y = f(x)\ny = x;` → `false`, and with `\nend` appended → `true`. Cases: complete_function_block, err_repl_function_body_not_run.
17. `disp(deep(400))\nfunction r = deep(n)\nif n <= 1, r = 1; else, r = 1 + deep(n - 1); end\nend` → `   400`: hundreds of frames fit the stack. Whether a limit of 500 admits exactly 500 frames or 499 is not settled by a source, so no case asserts the boundary itself. Cases: recursion_deep_frames.

## Status

Done (2026-09-28)
