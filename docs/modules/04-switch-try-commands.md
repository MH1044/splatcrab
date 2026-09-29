# 04 — Switch try commands

## Goal

`switch/case/otherwise` (numeric, char, `case {…}`), `try/catch e` with `e.message/identifier/stack` via a minimal struct, `error('id:x', fmt, …)`, `rethrow lasterr warning assert isequal`, command syntax (`hold on`, `format long`, `clear x y`), block comments `%{ %}`

After this module a user can do things that were not possible before; every
bullet in Scope must be demonstrable by at least one acceptance test below.

## Scope

- `switch/case/otherwise` (numeric, char, `case {…}`)
- `try/catch e`, where `e` is a minimal `MException` value rather than a
  struct, since structs arrive in cycle 07: `e.message` and `e.identifier`
  read its two texts, `class(e)` is `'MException'`, and `rethrow(e)` raises
  it again unchanged. Any other field is the Dot error cycle 03 defined for
  a value that has no fields. `e.stack` waits for cycle 05, since a stack
  means something only once user functions exist (the Errors key design in
  `docs/ARCHITECTURE.md`)
- `error('id:x', fmt, …)`
- The rest of `error`'s argument rules (QA D9), from the MATLAB `error` page.
  With one argument the message is literal, with no format or escape
  processing, so `error('100% sure')` reports `100% sure`. When every input is
  empty, `error('')` throws nothing. Today the first reports `100ure` and the
  second exits 1
- `rethrow lasterr warning assert isequal`
- command syntax, MATLAB's rule from its "Command vs. Function Syntax"
  page: a statement that starts with a name that is not a variable,
  followed by whitespace and then a word that is not an operator followed
  by whitespace, calls that name with each following word as a char
  argument. `clear x y`, `clear all` and `disp hello` work; `x -1` with `x`
  a variable stays an expression. `hold on` and `format long` are the same
  syntax, but `hold` and `format` are builtins of cycles 12 and 13, so
  today they are the ordinary unrecognized-name error, not a stub that
  accepts `long` and changes nothing
- `warning` writes to a second sink, `Interp.err`, beside `Interp.out`. In
  a script and at the REPL it is stderr. Under `--protocol` and `--ui` it
  is the same capture as `out`, so a warning appears in an `eval`'s `out`
  in the order it was raised, as a command window shows it, and the U0
  promise of nothing on stderr still holds. Cycle 11's `fprintf(2, ...)`
  writes to the same sink
- block comments `%{ %}`
- `syntax::is_complete`, which the REPL and U0's `complete` share, counts
  `switch` and `try` as block openers and an open `%{` as unfinished, so
  neither the terminal nor the browser page runs half a `switch` or half a
  block comment when Enter is pressed. `docs/ARCHITECTURE.md`'s "Add a
  statement" recipe names this step
- Displaying an `MException`, `e` with no semicolon, prints a short form of
  SplatCrab's own, recorded in the Design notes. MATLAB's property listing
  is not reproduced, and no golden case pins either (verify first)

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.
- `exist` (cycle 05), `format` (13) and `hold` (12), which command syntax
  will reach once they exist.
- `e.stack` and the `  in <fn> (line N)` trace, cycle 05.
- `inv` of a singular matrix warning instead of erroring: cycle 08 uses
  this cycle's `warning` to do it.


## Design notes

Filled in as decisions are made. Start from the
"Key designs to preserve" section of docs/ARCHITECTURE.md and record here:
which files and types changed, which invariants were preserved (column-major
storage, 1-based to 0-based conversion at the index boundary, the `end` stack,
name resolution order, the output sink), and any deviation from MATLAB that
was accepted deliberately.

### What changed, file by file

- `src/lexer.rs`: the tokens `Switch`, `Case`, `Otherwise`, `Try`, `Catch`,
  per the "Add a statement" recipe. Block comments. Command syntax, desugared
  on the spot into `Ident ( Str , Str ... )`, so the parser and the evaluator
  never see it. `scan_known(src, known)` takes a predicate for the names that
  are variables already; `scan` is it with none. `Lexed` gained
  `open_comment`.
- `src/parser.rs`: `Stmt::Switch(Expr, Vec<CaseArm>, Option<Vec<Located>>)`,
  `Stmt::Try(Vec<Located>, Option<String>, Vec<Located>)` and `CaseArm
  { values, line, body }`. `case`, `otherwise` and `catch` end a statement
  as `end` and `else` do, and each is `unexpected '<kw>' with no matching
  block` on its own.
- `src/interp.rs`: `exec_switch`, the `Try` arm, `Interp.err` with
  `emit_err`, `with_sinks(out, err)`, `last_err`, and `e.message` /
  `e.identifier` through the cycle 03 access chain (`exception_field`).
- `src/value.rs`: `Value::Exception(MError)`. `Value::mat` and
  `Value::into_mat` now return `R`, so every array operation refuses the
  exception with one message instead of each site deciding; `class_name`,
  `dims`, `display_body` and `disp_text` answer for both variants.
- `src/error.rs`: `MError.identifier` and `with_identifier`, and the texts
  `switch_expression`, `assertion_failed`, `no_method`, `not_an_array`,
  `expected_end_of` and `warning_line`; `raised` takes the identifier.
- `src/builtins/core.rs`: `error` rewritten on `message_args`, the one
  reading of MATLAB's message arguments that `error`, `warning` and `assert`
  share; `rethrow`, `lasterr`, `warning`, `assert`, `isequal`; `clear all`.
  93 builtins.
- `src/syntax.rs`: `is_complete` counts `switch` and `try`, and answers
  `false` for an open `%{`.
- `src/protocol.rs`: `eval` swaps `err` for the same capture as `out`.
  `src/main.rs` and `src/http.rs` build the `--ui` and `--http-stdio`
  interpreters over two sinks; a script and the REPL keep `Interp::new`,
  stdout and stderr.

### Invariants

Column-major storage and the one index boundary are untouched: nothing here
indexes. `end_stack` is saved at a `try` and truncated back when the handler
runs, although every path already pops it. Name resolution is unchanged;
command syntax only decides, before parsing, whether a statement is a call.
All output still goes through `Interp::emit`, and the warning through its
sibling `Interp::emit_err`, which flushes `out` first. Invariant 6: a nested
`switch` or `try` is a nested block, counted by `Parser::deepen` and
`Interp::deepen` like `if`; block comments are counted in a loop, not
recursed into; the command scanner, the assigned-name scan (once per
statement) and the marker test are linear.

### Choices where the Scope was silent

- **"Is a variable" is judged at lex time, statically.** The lexer knows the
  workspace the source starts in (a script's is empty; the REPL and the
  protocol pass the live one) and adds every name the source assigns as it
  goes: `x = `, `x(2) = `, `s.a = `, `for x = `, the names of `[a, x] = `,
  and `catch x`. This is how MATLAB treats a file (its "previously used as a
  variable" error is the same analysis); a name cleared earlier in the same
  script and then used in command form stays an expression. MATLAB's
  "conflicting use" error is not reproduced.
- **Where a command can start**: after a newline, `;` or `,` with no bracket
  open, at the start of the source, and after `else`, `try` and
  `otherwise`. Never the name after `catch`.
- **The command rule in detail**: after the name, at least one space or tab;
  then not a newline, `,`, `;`, `%`, `(` or a `...` continuation, and not a
  lone `=` (so `x =1` assigns). An operator (`== ~= <= >= && || .* ./ .\ .^`
  or one of `+ - * / \ ^ < > & | : =`) followed by whitespace or the end of
  the line makes an expression; followed by anything else it starts a word,
  so `x -1` and `ls ./d` are commands when the name is not a variable, as the
  MATLAB page says. Words end at whitespace; a `'...'` part of a word groups
  (with `''` for a quote) and is taken without its quotes; the command ends at
  a newline, `,`, `;` or `%` outside quotes. Double quotes are ordinary
  characters in a word. An unterminated quote is the lexer's
  `unterminated string`.
- **`clear all`** clears every variable, as a bare `clear` does; any `'all'`
  among `clear`'s arguments does.
- **Block comments**: the marker test is on the whole line with spaces, tabs
  and a carriage return trimmed. A `%}` with no opener is an ordinary comment.
  An unterminated `%{` runs to the end of a script as a comment, silently and
  with exit 0, the choice the Scope asked to record; at the REPL and under
  `complete` it is an unfinished entry, so the prompt keeps reading, and a
  REPL whose input ends inside one reports the unterminated-block error.
- **`switch` matching**: the subject is a scalar that is not a char, or a
  char with at most one row (so `''` is a character vector). A number
  matches a non-char scalar of equal value, whatever its class (`switch
  true, case 1` matches); a char matches a char of the same text; nothing
  else matches, and a non-scalar numeric case is simply no match. Case values
  are evaluated in order and only until one matches. An error in a case value
  reports the `case`'s line.
- **`case {…}`**: the lexer treats that brace as a bracket, so whitespace,
  commas, semicolons and newlines all separate values, and every value is
  matched. A `{` anywhere else is still cycle 03's brace index or a parse
  error.
- **`switch` arms**: only terminators may sit between the subject and the
  first `case`; anything else is `expected 'end' to close 'switch', found
  ...`, the `if` message generalised, and so is a second `otherwise` or a
  `case` after it. A `switch` with no arms is legal and does nothing.
- **`catch` binding**: an identifier directly after `catch` binds; it must be
  followed by a separator or `end` (`catch e f` is `unexpected 'f'`). A
  `try` with no `catch` swallows the error.
- **What `try` catches**: every error its body raises at run time, the
  nesting limit included; the nesting counters and `end_stack` are put back
  first. A parse error anywhere is raised before anything runs and is never
  caught, as in MATLAB. An error in the handler is not caught by its own
  `try`. A `break` outside any loop inside a `try` is a run-time error and is
  therefore caught.
- **The `MException` value** holds the caught `MError` whole. `rethrow`
  raises it unchanged, so an uncaught rethrow reports the line the error was
  first raised on, and `e.identifier` survives it. Errors the interpreter
  raises itself carry an empty identifier. `e.('message')` works as a
  dynamic field; `e(1)`, `e{1}`, arithmetic, concatenation and every numeric
  builtin refuse it: `Brace indexing is not supported for variables of this
  type.` for `e{1}` and `This operation is not supported for a value of
  class 'MException'.` for the rest, SplatCrab's own text (MATLAB names the
  operator or the function). `rethrow` of anything else is MATLAB's
  `Undefined function 'rethrow' for input arguments of type 'double'.`
  `isequal` of two exceptions compares message and identifier. `who` and the
  protocol's `workspace` list it as `1x1 MException`.
- **The `MException` display** (the short form the Scope asked for): `e`
  shows `e =`, a blank line, `  MException: <message>` (or
  `  MException (<identifier>): <message>` when it has one) and a blank line;
  `disp(e)` prints the middle line alone. No golden case pins it.
- **`error`**: `message_args` reads the arguments for `error`, `warning` and
  `assert`'s message. One argument is literal. With more, the first is the
  identifier when it contains a colon and no whitespace, and the rest are
  formatted as `sprintf` formats them, escapes included; otherwise all of them
  are. When every argument is empty, `error` and `warning` do nothing. A
  first argument that is not text keeps `error`'s old fallback message,
  `error`.
- **`warning`** writes `Warning: <msg>` and a newline to `Interp.err`,
  flushing `out` first; the identifier is not shown. `warning('off')`,
  `warning('on')` and `lastwarn` are not in Scope: `warning('off')` prints
  `Warning: off` (recorded in Known deviations).
- **`assert(cond)`** holds by the rule `if` uses: non-empty, every element
  non-zero, a `NaN` refused. `assert(cond, msg)` with one message argument is
  literal, by the same rule as `error`, and `assert(cond, 'id:x', fmt, ...)`
  attaches the identifier.
- **`isequal`** needs two arguments or more (`Not enough input arguments for
  'isequal'.`), compares sizes and values, ignores the class, and treats `NaN`
  as unequal to everything, as MATLAB's `isequal` does.
- **`lasterr`** is the message of the last error raised, whether a `try`
  caught it or it ended an entry; `''` before the first.

### Bytes settled in testing

- **Item 17**: the script-mode stderr of `warning('w'); disp(1)` is the 11
  bytes `Warning: w` and one LF, with `     1` and an LF on stdout and exit
  0. So `warning_in_protocol_out.out` is
  `{"id":1,"ok":true,"out":"Warning: w\n     1\n"}`: the warning line with
  its newline, then the `disp`, in the order raised, and nothing on stderr.
- **Script `.err` files** carry the `Error: Line N: ` prefix of the Errors key
  design, N counted from the file with its `% covers:` line as line 1. Where
  an error and the `rethrow` or `e.stack` that reports it share a line, N is
  that line whichever statement is named.
- **`err_nesting_try` and `err_nesting_switch`**: a nested `switch` or `try`
  counts against `parser::MAX_DEPTH` like `if`, so 12,000 levels is cycle
  01e's `Nesting is too deep. The maximum nesting depth is 10000.`, exit 1.
- **`err_repl_unterminated_switch`**: a REPL whose input ends inside an open
  `switch` reports `unterminated block`, as an open `for` does, with the
  banner, a blank line and one `>>` on stdout (cycle 01e's recorded bytes).
- **`block_comment_deep`**: 50,000 nested openings then 50,000 closings runs
  the `disp` after them, exit 0; no depth limit applies to a block comment,
  since it is counted, not recursed into.
- **Texts pinned here by the extra cases**, all recorded above or in
  `src/error.rs`: `Assertion failed.` (`assert([1 0 1])`), `Undefined
  function 'rethrow' for input arguments of type 'double'.` (`rethrow(5)`),
  `This operation is not supported for a value of class 'MException'.`
  (`e + 1`), `expected 'end' to close 'switch', found 'disp'` and
  `unexpected 'case' with no matching block`. `lasterr_message` covers
  `lasterr`, which has no Acceptance item: `''` before the first error, then
  the caught message `boom 1`.

### Deviations accepted

- The `MException` is minimal and displays in SplatCrab's own form; internal
  errors have no identifier (Known deviations).
- Command syntax judges variables statically, for a script and an entry
  alike (Known deviations).
- `hold on` and `format long` are the unrecognized-name error until cycles 12
  and 13, as the Scope says.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/04-switch-try-commands/`. Expected output is written by hand from MATLAB
semantics wherever feasible; the `.err` substring is always chosen by hand.
Every new error message needs an `err_*` case.

1. `x = 2; switch x, case 1, disp('one'), case {2, 3}, disp('two or three'), otherwise, disp('other'), end` → `two or three`. Cases: switch_cell_case, switch_otherwise, err_nesting_switch, err_switch_stray_statement, err_case_without_switch.
2. `s = 'abc'; switch s, case 'xyz', disp(1), case 'abc', disp(2), end; switch 5, case 'abc', disp(3), end` → `     2`. Cases: switch_char_case.
3. `for k = 1:5, switch k, case 3, break, end, fprintf('%d', k); end; fprintf('\n')` → `12`. Cases: switch_break_in_for.
4. `try, error('boom'), catch e, disp(e.message), disp(isempty(e.identifier)), end` → `boom\n   1` (`isempty` returns a logical, four wide since cycle 02). Cases: try_catch_message.
5. `try, error('MyPkg:myid', 'Value %d bad', 7), catch e, disp(e.identifier), disp(e.message), end` → `MyPkg:myid\nValue 7 bad`. Cases: try_catch_identifier, err_error_identifier, err_error_format.
6. `try, x = [1 2] * [3 4]; catch, disp('caught'), end; try, try, error('in'), catch e, rethrow(e), end, catch e2, disp(['outer: ' e2.message]), end` → `caught\nouter: in`. Cases: try_rethrow_nested, rethrow_keeps_identifier, err_rethrow_uncaught, err_nesting_try, err_rethrow_not_exception.
7. `try, undefined_thing + 1, catch e, disp(e.message), end` → `Unrecognized function or variable 'undefined_thing'.` (the wording since cycle 01e). Cases: try_catch_undefined.
8. `assert(true); assert(1 == 2, 'nope %d', 3)` → err `nope 3`. Cases: err_assert_message, err_assert_no_message.
9. `disp(isequal([1 2], [1 2])); disp(isequal('a', 'a', 'a')); disp(isequal([1 2], [1 2 3]))` → `   1\n   1\n   0` (logical). Cases: isequal_logical.
10. Command syntax: `x = 1; y = 2;\nclear x\ndisp(y)\nx` → `     2` then err `Unrecognized function or variable 'x'.`; `disp hello` → `hello`; `clear all` then `y` → the same error for `y`; `x = 3; x -1` → `ans =\n\n     2\n` (a variable, so an expression). Cases: err_command_clear_one, command_clear_two_words, command_disp_word, err_command_clear_all, command_variable_expression, err_command_format_unrecognized, err_command_hold_unrecognized (retired in cycle 12, when hold became a builtin; hold_on_close_all_commands in 12-plotting replaces it).
11. `%{\nthis is\na block comment\n%}\ndisp(1)` → `     1`. Cases: block_comment, block_comment_skips_code, block_comment_deep.
12. `switch [1 2], case 1, end` → err `SWITCH expression must be a scalar or a character vector.` Cases: err_switch_not_scalar.
13. `warning('careful %d', 1)` → stderr `Warning: careful 1`, exit code 0. Cases: warning_to_stderr.
14. `error('100% sure')` → err `100% sure`; `error('a\nb')` → err containing `a\nb` literally, the backslash and the `n` included. Cases: err_error_percent_literal, err_error_escape_literal.
15. `error(''); disp(2)` → `     2`, exit code 0. Cases: error_empty_no_throw.
16. `try, error('a:b', 'msg'), catch e, disp(class(e)), end` → `MException`; `try, error('x'), catch e, e.stack, end` → err `Dot indexing is not supported for variables of this type.` until cycle 05 (the text cycle 03 pinned). Cases: exception_class, err_exception_stack, err_exception_arithmetic.
17. Under `--protocol`, `{"id":1,"op":"eval","code":"warning('w'); disp(1)"}` → an `out` holding the warning line and then `     1\n`, and nothing on stderr. The warning line's exact text is item 13's `Warning: w`; pin its full bytes from the binary's script-mode stderr for the same call, not from recall. Cases: warning_in_protocol_out.
18. Under `--protocol`, `complete` on `switch x\ncase 1\n` → `false`; on `try\nx = 1;\ncatch\nend` → `true`; on `%{\nnot yet` → `false`. Cases: complete_switch_try_comment, complete_try_open_switch_closed, err_repl_unterminated_switch.

## Status

Done (2026-09-28)
