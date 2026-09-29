# 13 — Environment

## Goal

raw-console line editor via `kernel32`/`termios` FFI (no crate), history file, tab completion from registry + variables + path, `help which whos format eval evalc run pwd cd ls dir datestr now clock pause getenv system version exit`

After this module a user can do things that were not possible before; every
bullet in Scope must be demonstrable by at least one acceptance test below.

## Scope

- **The working directory is interpreter state, not the process's.**
  `Interp::cwd`, which cycle 05 introduced for its path lookups, readable
  and settable here, and every path-resolving builtin
  resolves against it. `cd` moves the interpreter, never `std::env`. This
  matters more than it looks: the golden harness gives each case its own
  working directory, and the interface confines the file pane to a root, so a
  builtin that moves the process out from under either one breaks both. It is
  also what lets `cd` in the command window move the file pane. Cheap to do
  here; expensive once every path builtin exists and has to be revisited
- **A raw-console line editor via `kernel32`/`termios` FFI (no crate). Kept:
  the project's owner decided on 2026-09-29 that the terminal REPL stays a
  first-class way to use SplatCrab.** Arrow keys move and recall history,
  Home/End, Backspace/Delete, Ctrl-C clears the line, Tab completes through
  `env::completions`. Built as a pure state machine (key events in; buffer,
  cursor and history index out) that unit tests drive, inside a thin
  raw-mode shell that restores the terminal on every exit path. When stdin
  is not a terminal, the REPL reads plain lines exactly as today, so every
  `.repl` golden case stays byte-identical
- **`splatcrab` as a command in any terminal**, as the project's owner asked
  on 2026-09-29: README install instructions
  for `cargo install --path .` and `cargo install --git
  https://github.com/MH1044/splatcrab`, which put `splatcrab` on the PATH
  through Cargo's bin directory; `--help` and `--version` flags (the version
  from `Cargo.toml` through `env!`); the no-argument REPL as the default,
  with the line editor. Prebuilt release binaries are not in scope: they
  publish something, which the owner decides separately
- history file, in a format the interface reads and writes too, so a terminal
  session and a browser session share one history rather than inventing two
- completion as a library function, `env::completions(prefix, vars, registry,
  path)`, used by both the terminal editor and the interface. U0 creates
  `src/env.rs` with the registry and variable halves, which are already
  reachable; this cycle extends it with path files. Tab completion in the
  terminal editor is then built on it rather than owning it
- `help which whos format eval evalc run pwd cd ls dir datestr now clock pause getenv system version exit`
- `exit` and `quit` as real statements everywhere (QA D28): in a script, as
  `exit;` or `quit;`, inside a block, and as `exit(n)`, which exits with code
  `n`. Today they work only as a bare REPL line, a script ending in `exit`
  fails with "Unrecognized function or variable 'exit'.", and the REPL's
  final exit code is always 0. Under `--protocol`, `--ui` and `--http-stdio`
  the session belongs to the client, so `exit` and `quit` are a clean error
  there (SplatCrab's own text) and never end the server
- `who` is bare names and `whos` a table of size, bytes and class (the Known
  deviations row scheduled here); bytes are 8 per double element, 1 per
  logical, 2 per char, twice for complex; the layout is SplatCrab's own,
  recorded in the Design notes
- `clc` writes the terminal clear only when stdout is a terminal, and nothing
  in a script, a pipe, `--protocol` or `--ui` (the Known bugs row scheduled
  here)
- `format short` and `format long` only, as interpreter state; `format`
  alone resets to short. Every existing case stays on short
- `pause(n)` sleeps with `n` bounded; `pause` with no argument waits for a
  key only at a terminal and is a clean error elsewhere, so a piped script
  never hangs
- `eval`, `evalc` and `run` go through the shared nesting budget; `evalc`
  captures by swapping the output sink, as U0 does; `system` runs a shell
  command and returns its status and output (the U1 token therefore guards
  a shell, which docs/modules/U1-ui-server.md's security notes must say)
- `env::completions` gains path files; the U0 completion cases' directories
  hold no `.m` file, so they are unchanged

Planning (2026-09-29) kept the line editor on the owner's decision, added
the terminal command, and added the bullets after `exit`: the non-terminal
modes, `who`/`whos`, `clc`, `format`, `pause`, `eval`, `system` and
completion.

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.
- A full-screen editor or debugger. Those are a later concern.

## Design notes

Filled in as decisions are made. Start from the
"Key designs to preserve" section of docs/ARCHITECTURE.md and record here:
which files and types changed, which invariants were preserved (column-major
storage, 1-based to 0-based conversion at the index boundary, the `end` stack,
name resolution order, the output sink), and any deviation from MATLAB that
was accepted deliberately.

Recorded during implementation (2026-09-29).

**Files and types.** New: `src/builtins/environ.rs` (the nineteen builtins
`cd pwd ls dir help which format eval evalc run datestr now clock pause
getenv system version exit quit`), `src/editor.rs` (`Key`, `Decoder`,
`Editor`, `Action`), `src/history.rs` (the history file), `src/term.rs`
(the binary's raw-mode shell, `LineReader`), `tests/cli.rs`. Changed:
`Interp` gains `format`, `stdout_tty` and `stdin_tty`, and the methods
`set_cwd`, `path_dirs`, `eval_code`, `run_path`, `display` and `disp_text`;
`value.rs` gains `Format` and `with_format`; `MError` gains the exit marker
(`MError::exit`, `exit_code`) in its boxed `Extra`, so the error stays the
size it was; `env::completions` takes the path; `core.rs` splits `who` from
`whos` and gates `clc`; `main.rs` gains `--help`, `--version`, the exit
codes and the editor. The registry grows from 231 to 250.

**Invariants.** Column-major storage, the one-based conversion in
`eval_index_args`, the `end` stack and the name resolution order are
untouched: `eval` runs its statements in the running frame, whose file's
local functions resolve first as they would for the calling code, and
`which` and `help` look a name up in invariant 4's order past the local
functions (variable, file on the path, builtin). The output sink is the
point of `evalc`: it swaps `Interp.out` and `Interp.err` for one buffer and
puts both back before an error inside is raised, as a protocol `eval`
does. Nothing outside `main.rs` and `term.rs` prints. `cd` never calls
`std::env::set_current_dir`.

**The exit signal.** `exit` and `quit` are builtins that return
`Err(MError::exit(n))`. An error already leaves every frame, loop, `feval`,
`cellfun` and solver cleanly, so no new control flow was needed; the marker
is what makes it not an error: `try` rethrows it before binding anything,
`eval(code, fallback)` does not run the fallback for it, `lasterr` does not
record it, and `main.rs` exits with its code in a script and at the REPL,
after flushing the output. Where the session belongs to a client the
refusal is decided by `Interp::input` being `Refused`, which
`protocol::eval` sets for the length of every call under `--protocol`,
`--ui` and `--http-stdio`: the same test that refuses `input`, so the two
can never disagree about which sessions are a client's. The refusal is
`exit is not available here: the client ends the session.` (or `quit`),
the text the case `err_exit_under_protocol` pins. `exit force` and `exit(n, 'force')` are accepted;
`n` must be a whole number from 0 to 255, the codes every platform passes
on unchanged, and anything else is `The code exit exits with must be a
whole number from 0 to 255.`

**The line editor.** Only when standard input and standard output are both
terminals; a pipe reads plain lines exactly as before, so every `.repl`
case is byte-identical, and nothing a test or CI runs reaches raw mode or
the history file. Raw mode lasts one line: it is entered when the prompt is
drawn and left, by a guard's `Drop`, before the line is returned, so the
entry runs, `input` reads and a panic unwinds with the terminal in its
normal mode. Keys: Left and Right (and Ctrl-B, Ctrl-F), Home and End (and
Ctrl-A, Ctrl-E, and the `1~`, `4~`, `7~`, `8~` forms), Backspace, Delete,
Up and Down (and Ctrl-P, Ctrl-N) through the history with the typed line
kept as a draft, Ctrl-C clears the line and drops an unfinished block,
Ctrl-D ends the session on an empty line and deletes otherwise, Tab
completes the name before the cursor through `env::completions`: one
candidate is inserted, several have their common part inserted, and when
that adds nothing they are listed. The drawing assumes one column per
character. A signal that kills the process while a line is being read
cannot restore the terminal; none is sent by any key, since raw mode makes
Ctrl-C a character.

**The history file.** `SPLATCRAB_HISTORY`, else `.splatcrab_history` in
`HOME` (or `USERPROFILE`). UTF-8, one entry per line, LF, oldest first;
`\` is `\\`, a line feed `\n` and a carriage return `\r`, any other
backslash pair read as written. The editor appends each submitted line
that is not blank or a repeat of the last; loading keeps the newest 1000
and rewrites a file past 2000 lines, so appends need no lock. Each line
typed is an entry, including the lines of a multi-line block. The browser
page still keeps its own history in memory: reading and writing this file
from the page needs a protocol operation, which is the interface's cycle,
so this cycle fixes the format and the functions it will use.

**`format`.** `short` is every display before this cycle. `long` keeps
every layout rule (integers, the fixed-point range from 0.01 to below
1000, `e` format for a scalar outside it, the scale factor for a matrix)
and writes fifteen decimals, in columns three wider than the widest
element not counting a minus sign: `   3.141592653589793`. Integers and
logicals are unchanged, and complex parts take fifteen decimals too. The
state is `Interp::format`, handed to the display for one display at a time
through `value::with_format`, a thread-local the display reads, so no
display signature changed. Anything but `short` or `long` (any case) is
`Unsupported format '<f>'; SplatCrab has 'short' and 'long'.`

**`who` and `whos`.** `who` is `Your variables are:`, a blank line, the
names in byte order two spaces apart, wrapped at 80 columns, and a blank
line. `whos` keeps the columns of the typed table `who` printed before and
adds bytes, with no heading row: each row is two spaces, the name padded to
the widest name and at least 12, a space, the size right-aligned to the
widest size, the bytes right-aligned in a field three wider than the
widest, two spaces and the class, then one blank line after the table:
`  s            1x2    4  char` and `  x            1x3   24  double`, the
rows the case `whos_table` pins. A heading could not align over both the
size and the bytes and keep the spacing the Acceptance test records. Bytes are 8 per double
element, 16 per complex one, 1 per logical, 2 per char; a cell or struct is
the sum of what it holds, walked with a worklist; a handle or an
`MException` is 0.

**`help` and `which`.** `help name` looks for a file on the path first, as
a call would, and prints its leading comment block: blank lines skipped, a
`function` line skipped, then each `%` line less its `%`, until the first
line that is not one (`%{` blocks are not help). With no block it prints
`No help found for name.`; a builtin prints its registry line as it is; a
name that is neither prints `'name' not found.` A bare `help` is `help
help`. `which` prints `x is a variable.`, a file's full path, `built-in
(sum)` or `'x' not found.`; with an output it returns `variable`, the path,
`built-in (sum)` or `''`. Neither reports local functions.

**`eval`, `evalc`, `run`.** `eval` asked for no outputs parses its text as
statements (a `function` refused), and asked for outputs as one expression;
it counts one level of `MAX_DEPTH`, parses from the evaluator's depth, runs
with no loop around it (a `break` in the text is the outside-a-loop
error), puts the depth back however it ends, and clears the text's line
from an error so the calling statement's line is reported.
`eval(code, fallback)` runs the fallback, with `lasterr` set, when the code
fails. `evalc` returns what was printed less one final line end: the spec
records `numel(evalc('disp(1)'))` as 6, so the text is `     1` without its
`\n`. `run(script)` takes a path with or without `.m`, resolved against
the current folder, or else a name on the path; it moves the interpreter to
the script's folder for the run and back however it ends, counts a level
of the nesting budget and, through `run_script`, the recursion limit; a
function file is called with no arguments.

**The folder builtins.** `cd` with no argument prints the folder;
`old = cd(dir)` returns the one it left. `cd` resolves against the current
folder and normalises `.` and `..` by their components, without asking the
file system, so a symbolic link is not followed. `Cannot CD to <dir> (Name
is nonexistent or not a directory).` `ls` and `dir` take a folder, a file,
or a pattern whose last part has `*` or `?`, and print one name per line in
byte order with `.` and `..` left out; `s = ls` is a padded char matrix,
`s = dir` an Nx1 struct of `name`, `folder`, `bytes` and `isdir` (no `date`
or `datenum`, which need the file time in local time). A folder that
cannot be listed is `Cannot list '<dir>': no such folder.`

**Time.** `now` and `clock` read local time through `GetLocalTime`
(Windows) or `localtime_r` (64-bit Unix), raw declarations like the
terminal's, with UTC elsewhere. Date numbers count days from MATLAB's year
0 (`719529` is 1970-01-01), through Hinnant's civil-date algorithms.
`datestr(d)` writes each date number, or one 1x6 date vector, as
`dd-mmm-yyyy HH:MM:SS` rounded to the second; it takes no format argument,
and anything else is `datestr needs date numbers, or a date vector of 6
elements.` `pause(n)` takes 0 to 86400 seconds (`The pause time must be a
real number of seconds from 0 to 86400.`), flushing output first; a bare
`pause` waits for Enter, not any key, since standard input stays in its
normal mode while code runs, and without a terminal it is `pause with no
argument waits for a key, and there is no terminal to read one from.`

**The process's surroundings.** `getenv` returns `''` for an unset
variable or a name no variable can have. `system(cmd)` runs `cmd /C` (the
text passed verbatim) or `sh -c` in the current folder with standard input
closed, so it can never read a script's input or a protocol's requests;
its status is the first output (`-1` for a process ended by a signal) and
its standard output then standard error, decoded as UTF-8, the second.
Asked for fewer than two outputs it prints the text. `version` is
`Cargo.toml`'s version. Output that depends on the machine (a path, a date,
a command's output) is never pinned by a case; the cases assert its type
or a property of it.

**`clc`** writes `ESC[2J ESC[H` only when `Interp::stdout_tty` is set,
which `main.rs` sets from `IsTerminal` for a script and the REPL, and which
is false under the protocol and for the length of an `evalc`. A script run
at a terminal therefore does clear it, as MATLAB's `clc` clears the command
window; a script whose output is piped or redirected writes nothing.

**The binary.** `--help` and `--version` are answered before anything
else, each exiting 0; `tests/cli.rs` checks both and a piped REPL's
`exit(7)`. The banner's version comes from `env!` too, so it reads as it
did.

**Deviations from MATLAB accepted.** All of the above that is marked as
SplatCrab's own; in short: `format` has two formats; `whos` has no
heading; `ls` and `dir` print one name per line with no `.` or `..` and
`dir` has no dates; a bare `pause` waits for Enter; `evalc` drops its final
line end; `datestr` has one format; `exit(n)` is 0 to 255; `which` and
`help` skip local functions; `help` of a builtin is its one registry line.

**Settled in testing (2026-09-29).** `evalc` keeps dropping its final line
end: the Acceptance test records 6 for `numel(evalc('disp(1)'))` and the
spec is the authority. `help sum` prints the registry line exactly as it
is (`sum(A), sum(A,dim), sum(A,'all') - sum of the elements.`), not the
`SUM Sum of elements.` the Acceptance test gives as an example. `who` ends
with a blank line after the names and `whos` with one after the table
(`who_bare_names` and `whos_table`, whose trailing blank lines the harness
normalises). The other texts pinned here: `Cannot run '<name>': no such
script file.` (`err_run_missing_script`); `'<name>' not found.`, `No help
found for <name>.` and `<name> is a variable.` for `help` and `which`
(`help_unknown_name`, `which_unknown_name`, `help_file_without_comments`,
`which_variable`); each error text above in an `err_*` case, with `quit`
under `--protocol` and `exit` under `--http-stdio` refused in the same
words. `Cannot run the command: <reason>`, for a shell that cannot start,
has no case: `cmd` and `sh` always start on a machine that runs the suite.
Further cases beyond the numbered items: `format_bare_resets_short`,
`whos_logical_complex_bytes`, `run_helper_script`,
`system_status_and_output`, `dir_struct_fields`,
`clock_getenv_version_types`, `exit_inside_try`, `exit_inside_function`,
`eval_fallback_on_error`, `cd_dotdot_returns`, `err_cd_to_a_file`,
`err_pause_too_long`, `err_eval_recursion_limit` and
`err_eval_function_definition`. `err_command_format_unrecognized` in
04-switch-try-commands is retired; `format_long_then_short` replaces it.

**Fixed at review.** `datestr` of a date vector or a date number past the
range of its integer arithmetic panicked (`datestr([1e17 1 1 0 0 0])` exited
101) or printed a nonsense year (`datestr(1e300)`). Every component and every
resulting date number is now bounded at `MAX_DATE`, 1e10 days, and refused
past it with `Dates for 'datestr' must lie within 1e+10 days of year 0.`
(`err_datestr_out_of_range`). Also: `pause` refuses a char or logical
argument, which `pause('a')` read as 97 seconds; and `exit` takes at most
one code, where `exit(1, 2)` exited 2.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/13-environment/`. Expected output is written by hand from MATLAB
semantics wherever feasible; the `.err` substring is always chosen by hand.
Every new error message needs an `err_*` case.

1. `format long; disp(pi); format short; disp(pi)` → `   3.141592653589793\n    3.1416` Cases: `format_long_then_short.m`.
2. `help sum` → the registry help line (e.g. `SUM Sum of elements.`). Cases: `help_builtin_registry_line.m`.
3. Helper `myf.m` beginning `% MYF does things\n% second line`; `help myf` → ` MYF does things\n second line` Cases: `help_file_leading_comments.m`.
4. `eval('q = 6 * 7;'); disp(q); s = evalc('disp(1)'); disp(numel(s))` → `    42\n     6` Cases: `eval_assigns_in_workspace.m`, `evalc_captures_output.m`.
5. `which sum` → `built-in (sum)`; `disp(~isempty(strfind(which('addone'), 'addone.m')))` → `   1` (an absolute path is machine-dependent, so the case asserts only that it names the file) Cases: `which_builtin.m`, `which_path_file.m`.
6. `x = 1:3; s = 'ab'; whos` → locked two-column table (`1x3   24  double`, `1x2    4  char`). Cases: `whos_table.m`.
7. `disp(ischar(pwd)); disp(ischar(datestr(now)))` → `   1\n   1` (logicals, four wide since cycle 02) Cases: `pwd_is_char.m`, `datestr_now_is_char.m`.
8. Unit tests for the line-editor state machine (key events → buffer/cursor/history index) and history-file round trip.
9. `t = tic; pause(0.05); disp(toc(t) >= 0.04)` → `   1` Cases: `pause_bounded_sleep.m`.
10. REPL transcript: pipe `1+1\nx = 3;\nx\nexit\n` into the binary with no args; lock the transcript as a golden. Cases: `repl_transcript.repl`.
11. A script `disp(1)\nexit\ndisp(2)` → `     1`, exit code 0; a script `disp(1)\nexit(3)` → `     1` and exit code 3. The golden harness expects exit 0 without an `.err` file, so this case needs an exit-code expectation added to it in this cycle. Cases: `exit_in_script.m`, `exit_with_code.m`, `quit_inside_block.m`.
12. `splatcrab --version` prints `SplatCrab <version from Cargo.toml>` and `--help` a usage text naming `--protocol`, `--ui` and a script argument, both exit 0: an integration test in `tests/cli.rs`, since golden cases run with fixed flags
13. Under `--protocol`, `{"id":1,"op":"eval","code":"exit"}` → an error answer (SplatCrab's text pinned in the case), and a following request is still answered Cases: `err_exit_under_protocol.proto`.
14. `x = 1; s = 'ab'; who` → the bare names; `clc` in a script writes nothing to stdout (`.out` empty apart from what the case prints) Cases: `who_bare_names.m`, `clc_script_writes_nothing.m`.
15. `mkdir`-free directory test: a helper folder `envdir/` beside the case holding one file `a.txt`; `cd envdir; ls; cd ..` → `a.txt`, and `disp(strcmp(pwd, pwd))` → `   1` Cases: `cd_ls_envdir.m`, `pwd_strcmp_self.m`.

## Status

Done (2026-09-29)
