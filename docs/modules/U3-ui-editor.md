# U3 — UI editor

## Goal

The desktop gains an editor: files open in tabs with line numbers, save
back to disk, run whole or by selection, and an error takes you to the line
that raised it. The file operations are confined to U2's file root by the
same rule as `files`, every byte they answer is pinned by golden cases, and
the one new mutating operation is guarded by one more browser check. The
standard library only: the editor is a `<textarea>` and a gutter written
here, with no editor component.

After this module a user can do things that were not possible before; every
bullet in Scope must be demonstrable by at least one acceptance test below.

## Scope

- **The editor region** sits above the command window in the middle column,
  with a fourth splitter between them (pointer and arrow keys, 16 px, the
  120 px minimum, as U2's). A tab bar across its top, one tab per open file,
  each with the file's name, a `•` while it has unsaved changes, and a close
  button. Below it a line-number gutter beside a `<textarea>` that does not
  wrap, the two kept in step as the text scrolls, grows and shrinks; the
  gutter is rebuilt only when the number of lines changes. With no tab open
  the region shows a one-line hint. In the one-column layout it follows the
  command window
- **Opening.** Double-clicking a file in the file browser opens it in a new
  tab, or selects its tab when it is open already. New opens an empty tab
  named `untitled.m`, `untitled2.m`, and so on, which is saved by naming a
  path under the root in an in-page field (never `window.prompt`)
- **Editing keys.** Tab inserts four spaces at the cursor; Shift+Tab removes
  up to four spaces at the start of each selected line; Escape lets the next
  Tab move the focus on, so the keyboard can always leave the editor.
  Ctrl+S (Cmd+S on a Mac) saves, F5 saves and runs, F9 runs the selection.
  Line ends are kept: a file read with CRLF line ends is written back with
  CRLF, since the `<textarea>` itself holds LF only
- **Unsaved changes.** Closing a tab with unsaved changes asks in the page
  (Save, Discard, Cancel), never with `window.confirm`; leaving or reloading
  the page with any unsaved tab triggers the browser's own `beforeunload`
  warning. Nothing is saved without the user asking
- **`read_file`**: `{"id":1,"op":"read_file","path":"prog/s2.m"}` answers
  `{"id":1,"ok":true,"path":"prog/s2.m","text":"..."}`, the file's text
  exactly, `path` normalised as `files` normalises it. `path` is required, a
  string. It is judged by U2's confinement rule, steps 1 and 2 unchanged; at
  step 3 a path that cannot be canonicalised, is the root, or is a folder is
  `Path '<path>' is not a file.`, and one outside the root is U2's outside
  message, judged first. A file larger than 4 MiB (4,194,304 bytes), judged
  from its length before a byte is read, and read no further than that bound
  plus one, is `Path '<path>' is larger than 4 MiB.`; one that is not valid
  UTF-8 is `Path '<path>' is not UTF-8 text.` A UTF-8 byte-order mark is
  part of the text and comes back when the text is written
- **`write_file`**: `{"id":2,"op":"write_file","path":"prog/s1.m","text":"..."}`
  answers `{"id":2,"ok":true,"path":"prog/s1.m","size":123}`, `size` the
  bytes written. The fields are judged first, `path` then `text`, each
  required and a string. Then steps 1 and 2 as U2's; a path with no
  component left is `Path '<path>' is not a file.`, and so is one whose last
  component is a Windows device name (`CON`, `PRN`, `AUX`, `NUL`, `COM0` to
  `COM9`, `LPT0` to `LPT9`, in any case, alone or before a `.`) or ends in a
  `.` or a space, on every platform, so a name always names the file it
  says. Then the folder the file is in, every component but the last, is
  canonicalised: missing or not a folder is `Path '<path>' is not in a
  folder of the file root.`, outside the root the outside message. Then the
  file itself: when anything exists at that name, a link included, it is
  canonicalised and must be inside the root (else the outside message) and
  not a folder (else `Path '<path>' is a folder.`), and a link that resolves
  nowhere is `Path '<path>' is not a file.`, so a write never follows a link
  out of the root or creates a file at a dangling link's target. Last, a
  `text` longer than 4 MiB in UTF-8 is `Text for '<path>' is larger than 4
  MiB.` The text is written as sent, as UTF-8, creating the file or
  replacing it; a failure is `Path '<path>' could not be written.`, with no
  operating-system text. No folder is ever created
- **The file cache forgets what the interpreter writes.** A write by
  `write_file`, or by any builtin that already calls
  `Interp::files_changed` (`fclose` of a file opened for writing, `save`,
  `delete`, and the rest), drops every parsed file the interpreter holds,
  so the next call of a function reads its file again. Today the cache
  keeps a parse whose file's modification time and length are unchanged,
  and a file rewritten with the same length within the file system's
  timestamp tick is then run from its old text: a loop that writes
  `function ff` with `disp(k)` and calls `ff` printed `3` twice and never
  `4`. Save followed at once by Run meets exactly that
- **`run_file`**: `{"id":3,"op":"run_file","path":"prog/s1.m"}` runs the
  file as `run` runs a script file named by its full path, in the base
  workspace, a function file called with no arguments, and answers as
  `eval` does, `out` then `error`, with the error's `line` `null` (no code
  was submitted) and its `stack` always present. `path` is required, a
  string, judged as `read_file` judges it, and must end in `.m`, else `Path
  '<path>' is not a .m file.` Output, warnings and `input` are captured and
  refused exactly as an `eval`'s, and the file is run from the root, not
  from `Interp::cwd`, wherever `cd` has gone
- **`eval` with `"stack": true`** adds a `stack` key after `line` to the
  error object of a failed `eval`. Without the field, or with `false`, the
  answer is exactly U0's, byte for byte, so the cases of 05, 11 and 13 that
  pin error answers are unchanged; any other value is `Malformed request:
  'stack' must be true or false.` The page always sends it
- **The stack** is the error's frames innermost first, the frames
  `e.stack` holds:
  `[{"file":"prog/s1.m","name":"helper","line":6},{"file":"prog/s1.m","name":"s1","line":3}]`.
  `file` is the frame's file relative to the root with `/` separators,
  judged on its canonical path, or `null` when the file is outside the root
  or the frame has none (a function local to the submitted code, an
  anonymous function); `name` is the frame's name as the trace writes it,
  an anonymous function's being its `func2str` text; `line` is `null` when
  the frame has none. An error with no frames has `"stack":[]`
- **Run and Run Selection.** Run (the button, or F5) saves the tab if it has
  unsaved changes, then sends `run_file`, and the command window shows the
  entry as `run('<path>')`. Run Selection (the button, or F9) sends the
  selected text, or the cursor's line when nothing is selected, as one
  `eval`, shown in the command window as typed; a `function` in it is
  refused with the existing message. Both refresh the workspace and the file
  browser afterwards, as an entry typed in the command window does, and add
  the entry as shown to the history
- **An error jumps to its line.** After Run, the page takes the innermost
  frame whose file is open in a tab, or failing that the frame of the file
  that was run, activates that tab, puts the cursor at the start of the line
  and marks the line in the gutter until the text is next edited. After Run
  Selection an error's `line` counts from the selection's first line. In
  the command window every error with a stack lists its frames under the
  message, `in helper (line 6)`, and a frame with a file under the root is a
  link that opens the file at that line
- **`Sec-Fetch-Site`.** A `POST /api` whose `Sec-Fetch-Site` header is
  present and is not `same-origin` is `403 Forbidden` and never reaches the
  interpreter, judged after the token and before the content type. Browsers
  send the header on every fetch, so a request another site makes is
  refused even if it somehow carried the token; a client that sends no such
  header, a golden case or a script, is unaffected, so every U1 case stands.
  Two such headers fail the check. Static routes are not checked, since a
  navigation to the page is not same-origin
- The REPL, script mode and every existing operation behave exactly as
  before, and every existing case passes unchanged

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.
- Syntax highlighting, find and replace, undo beyond the browser's own,
  breakpoints and a debugger.
- Noticing a file changed on disk while it is open; reopening the page's
  tabs after a reload.
- Creating, renaming or deleting files or folders from the page, beyond
  saving a new file into an existing folder.
- A file changed by another program with the same length within the
  timestamp tick: the cache cannot see it, and only a read of every file at
  every call could.
- Inline figures, Tab completion in the page and the file browser following
  `cd`: cycle U4.
- Text in any encoding but UTF-8.
- An automated browser test. The page is checked by hand at the close of the
  cycle, and the commit says what was checked.

## Design notes

Filled in as decisions are made. Start from the
"Key designs to preserve" section of docs/ARCHITECTURE.md and record here:
which files and types changed, which invariants were preserved (column-major
storage, 1-based to 0-based conversion at the index boundary, the `end` stack,
name resolution order, the output sink), and any deviation from this spec
that was accepted deliberately.

Settled at planning:

- **`stack` rides behind a flag on `eval`,** as U2's `preview` rides on
  `workspace`, because always adding it would change the bytes of cycle 05's
  `err_eval_error_line_outermost`, whose error comes from inside a function
  file. `run_file` is new, so it always carries it.
- **Run is `run_file`, not an `eval` of `run('...')` typed by the page.** The
  page knows paths relative to the root, and `run` resolves against
  `Interp::cwd`, which `cd` moves; the server resolves the path against the
  root and runs the file by its full path, so Run works wherever `cd` has
  gone, and a path holding a quote needs no quoting.
- **The write is judged before it is made,** folder, then file, then text,
  and the window between the check and the write is not defended, as U2's
  listing window is not: changing a link inside the root in that moment
  needs write access to the root, which is outside the threat model.
- **The first mutating operation.** `write_file` can replace any file under
  the root, a `.m` file the next `run` executes included. `eval` could
  already do as much through `fopen` and `fprintf`, so the token was already
  the guard of the file system; `Sec-Fetch-Site` is one more check on top of
  it, and neither may be relaxed.
- **The cache fix is in this cycle** because Save then Run is this cycle's
  core, and a probe showed the stale parse with the existing builtins: it is
  a defect of the cache, found at this cycle's planning, not a Known bugs row.
  Dropping every parse on a write costs a re-parse of the files next called,
  which a script writing files in a loop pays once a pass.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/U3-ui-editor/`, as a `.proto` case unless it says otherwise.
Expected output follows from this spec's rules and U0's recorded bytes.

The fixture folder `tests/cases/U3-ui-editor/prog/` holds `s1.m`, the seven
lines `disp('start')`, `b = 2;`, `helper(3)`, `function helper(x)`,
`  y = x + 1;`, `  z = nosuch(y);` and `end`; `s2.m`, the two lines
`q = 1;` and `w = undefined_thing + 1;`; and `crlf.txt`, the six bytes `a`
CR LF `b` CR LF, which `.gitattributes` marks binary so no checkout changes
them. A case that writes a file writes a name starting `scratch_` in the
case folder and deletes it before it ends with an `eval` of `delete`;
`.gitignore` ignores `tests/cases/U3-ui-editor/**/scratch_*`, so one left
by a failed run is never committed.

1. `read_file` of `prog/s2.m` → `"text":"q = 1;\nw = undefined_thing + 1;\n"`, and of `prog/./crlf.txt` → `"path":"prog/crlf.txt","text":"a\r\nb\r\n"`.
2. `write_file` of `scratch_a.m` with the text `disp(7)` LF `disp(8)` LF → `"size":16`; `read_file` of it → the same text; `run_file` of it → `"out":"     7\n     8\n"`; then an `eval` of `delete('scratch_a.m')`.
3. A function file rewritten at once with the same length: `write_file` of `scratch_f.m` with `function scratch_f` LF `disp(1)` LF `end` LF, `run_file` → `     1`; the same with `disp(2)` → `     2`; and an `eval` of a loop that writes `scratch_g.m` with `fprintf` five times, each with `disp(k)`, calling `scratch_g` after each write → `1` to `5`, each once. Then both deleted.
4. `read_file` refusals: `..` → outside; `/etc` → malformed; `""` and `prog` → not a file; `nope.m` → not a file; `path` missing → no 'path' field.
5. `write_file` refusals: `..` → outside; `nofolder/x.m` → not in a folder of the file root; `prog` → is a folder; `NUL.m`, `con`, `x.m.` and `x.m ` → not a file; `text` missing → no 'text' field; `"text":3` → 'text' must be a string. None creates anything: a following `files` of `prog` lists exactly its three files.
6. `run_file` of `prog/s1.m` → `"out":"start\n"`, the message `Unrecognized function or variable 'nosuch'.`, `"line":null`, and `"stack":[{"file":"prog/s1.m","name":"helper","line":6},{"file":"prog/s1.m","name":"s1","line":3}]`; of `prog/s2.m` → `"out":""` and the one frame `{"file":"prog/s2.m","name":"s2","line":2}`; of `prog/crlf.txt` → `Path 'prog/crlf.txt' is not a .m file.`
7. `run_file` after an `eval` of `cd prog` still runs `prog/s2.m` from the root (the same answer as in 6), and an `eval` of `cd ..` restores the folder.
8. `eval` of `run('prog/s1.m')` with `"stack":true` → `"out":"start\n"`, `"line":1` and the same two frames; the same without the flag → exactly U0's form, with no `stack` key; `"stack":1` and `"stack":"yes"` → `Malformed request: 'stack' must be true or false.`
9. `eval` with `"stack":true` of `x = nosuchname` → `"stack":[]`; of `f = @(n) nosuch(n); f(1)` → `"stack":[{"file":null,"name":"@(n)nosuch(n)","line":null}]`.
10. `.http` cases: `POST /api` with the token and `Sec-Fetch-Site: cross-site`, `same-site` and `none` → `403 Forbidden` each, a following `disp(x)` showing none was evaluated; with `Sec-Fetch-Site: same-origin` → `200 OK`.
11. Unit tests: the 4 MiB bounds for reading (a file of 4,194,304 bytes read, one of 4,194,305 refused without reading it) and for writing (a text of 4,194,304 bytes written, one byte more refused); a file that is not UTF-8 refused; on Unix, a dangling link and a link out of the root refused by `write_file` with nothing created; every device name and trailing character refused; the cache dropped by `files_changed`.
12. `tests/ui_server.rs`: a `write_file`, `read_file` and `run_file` round trip over the socket in a fixture folder the test makes, and a `Sec-Fetch-Site: cross-site` request refused.
13. Every existing case passes unchanged.
14. A hand check in Chrome: open a file from the file browser; edit it (the `•` appears); save with Ctrl+S (it goes); change one digit and press F5 (the new digit runs); Run Selection with F9; an error in a local function moving the cursor to its line in the right tab; a stack link in the command window opening the file at its line; New, then save under a name; closing an unsaved tab asking in the page; the gutter keeping step with a long file; Tab, Shift+Tab and Escape then Tab.

## Status

Planned
