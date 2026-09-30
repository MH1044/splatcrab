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
  `write_file`, or by any builtin that writes or deletes a file, as those
  that called `Interp::files_changed` before this cycle did (`fopen` for
  writing, `fclose` of a file opened
  for writing, `save`, `delete`, and the rest), drops the parse of the
  file it wrote, the same file by canonical path, and every cached lookup
  of a name to a file, so the next call of a function in that file reads
  it again and a file just made is found; the parses of every other file
  are kept, judged by their modification time and length as before, since
  dropping them all made a loop that writes a log file and calls a large
  helper fifty times slower (settled at review). Today the cache
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
  Dropping only the written file's parse costs nothing for the files the
  write did not touch.

Settled at review, before the code that follows it:

- **The cache drops one parse, not all.** The first build dropped every
  parse on every write, as this spec then said; the review timed a loop of
  300 writes to a log file calling a 118 KB helper at 0.41 s before the
  cycle and 20.5 s after. The Scope bullet now drops the written file's
  parse and the lookups alone.
- **`Path '<path>' could not be written.`** gets its golden case through a
  file name of 300 characters, which no common file system takes: Windows
  refuses the write, and Linux and macOS refuse the look at the name
  (`ENAMETOOLONG`), which is not "not found", so both answer the message.
- **`Text for '<path>' is larger than 4 MiB.`** is pinned by a unit test
  only, as acceptance item 11 plans: a golden case would need a request
  line over 4 MiB in the repository.
- **Accepted residues.** U2's order of checks, which this spec inherits,
  still says whether a name exists beyond a link that leads out of the
  root (`jout/missing/x.m` is "not in a folder", `jout/existing/x.m`
  "outside"); a token holder learns as much through `eval`. `run_file`
  reads a `.m` file whole, as `run` and every call of a function file
  always have. U1's 8 MiB body cap means a 4 MiB text made mostly of
  characters JSON escapes cannot be saved over HTTP, though `--protocol`
  takes it; the page shows the server's `413` as any refusal.

Recorded during implementation.

**Files and types.** Changed: `src/files.rs` gains `read_file(root, path,
bound)`, `write_file(root, path, text, bound)`, `run_file(root, path)` and
`relative(root, file)`, the types `FileText`, `Written` and `Runnable`, the
constant `MAX_TEXT` (4,194,304), and the private `joined` (the one push of
the relative path that `list` now shares), `existing_file`,
`read_bounded`, `is_device`, `names_itself` and `plain`; `src/protocol.rs`
gains `read_file`, `write_file`, `run_file`, `eval`'s `stack` through a
flag function `workspace`'s `preview` now shares, `root_for`, `captured`
(the capture of `out`, `err` and `input` that `eval` had inline),
`evaluated`, `stack_json` and `line_json`; `src/interp.rs` gains
`Interp::run_file` and the private `start_entry` and `finish_entry`, which
`run_with` now calls, the type `FileId`, a `FileId` on each `CachedFile`,
and `Interp::file_written(path)` and `Interp::file_written_as(path, id)`,
which replace `Interp::files_changed` (see "The cache" below);
`src/builtins/io.rs`'s `OpenFile` gains the full path it was opened at and
its `FileId`, and `files::Written` gains `file`, the file written;
`src/http.rs`
gains `same_origin_fetch`, checked in `handle` between the token and the
content type; `src/error.rs` gains `request_stack`, `file_not_a_file`,
`file_too_large`, `file_not_utf8`, `file_no_folder`, `file_is_a_folder`,
`text_too_large`, `file_not_written` and `file_not_m`; `src/ui/` gains the
editor region, the fourth splitter and the stack's links;
`tests/ui_server.rs` gains `the_editor_operations_answer_over_the_socket`.
`tests/golden.rs` is unchanged.

**Invariants.** The evaluator's column-major storage, the one-based
conversion in `eval_index_args`, the `end` stack and the name resolution
order are untouched. The output sink is untouched: `run_file` captures
through the same swap of `Interp.out`, `Interp.err` and `Interp.input` that
`eval` uses (`captured`), and nothing reaches the writer but a response.
Invariant 6: a read judges the file's length from the open handle's
metadata before a byte is read and reads through `take(bound + 1)`, so it
holds at most 4 MiB and one byte whatever the file does meanwhile; a write
judges the text's length before it opens anything; a path is judged in
time linear in its length, by U2's `normalise` and one join; a stack
canonicalises each distinct file once, however many frames name it (a
recursion at the limit is 500 frames of one file). Neither operation opens
anything that is not a regular file, so a device or a pipe inside the root
cannot make a read or a write wait for ever. No `unwrap` or `expect` reads
anything a request or the file system controls.

**Choices where the spec was silent.**

- **The order of the checks.** `read_file`: the `path` field, then steps 1
  and 2, then step 3 for a file (not canonicalisable, then outside, then
  not a regular file), then the length, then the read, then UTF-8.
  `run_file`: as `read_file` to step 3, then the `.m` rule; neither the
  4 MiB bound nor UTF-8 is a rule of `run_file`, which reads the file as
  `run` reads it. `write_file`: exactly the spec's order, and a failure to
  look at the name for any reason but its absence, which leaves no way to
  know what is there, is `could not be written`. `eval`: `code` first, then
  `stack`, so `{"op":"eval","stack":1}` is `no 'code' field`. With no file
  root, which only a test builds, the text is judged and every
  well-formed path is outside the root, as `files` does.
- **A file that is not a regular file**, a device or a pipe inside the
  root, is `not a file` for all three operations: a read of it could wait
  for ever. A file that cannot be opened or measured (locked by another
  program) is `not a file` for `read_file`, the only message the spec has
  for it.
- **A device name before a `.` with spaces between**, `NUL .m`, is refused
  as the device it is on Windows: the part before the first `.` is judged
  with its trailing spaces dropped, as Windows drops them. `CONIN$`,
  `CONOUT$` and `CLOCK$` are not on the spec's list and are not refused.
- **The `.m` rule** is judged on the last component of the normalised
  path, case-sensitively as `run` judges it (`prog/s1.m/` and
  `prog/./s1.m` are runnable; `S1.M` is not a `.m` file), and a refusal of
  `run_file` is a refusal like any other operation's, `{"id","ok":false,
  "error":{"message","line":null}}`, with no `out` and no `stack`, since
  nothing ran.
- **The path run.** `run_file` runs the file by the root joined to the
  normalised path, not by its canonical path, so a link keeps its own name
  (`name` is the last component less its extension, as `run` names a
  file). On Windows the canonical root is a verbatim `\\?\` path, which
  does not read `/` as a separator; the path is run in its plain form
  (`C:\...`, or `\\server\share\...` for `\\?\UNC\`) when that
  canonicalises to the same file, and in the verbatim form otherwise. So a
  script run with F5 sees its folder in `pwd` as it would from the
  terminal and can build a path to its data with `[pwd '/data.csv']`.
- **The stack's file** is the frame's recorded file canonicalised and cut
  to the root, each component that is not Unicode making it `null`, as a
  `files` listing leaves such a name out. An anonymous function made in a
  file under the root records that file, as `e.stack` does, so its frame
  has the file and a `null` line; one made in the submitted code has
  `null` for both. A frame of a parse error in a file on the path is
  recorded by `Interp::load` without its file, as before this cycle, so its
  file is `null`; the page's Run falls back to the outermost frame, which
  is always the run file's own, to find the line.
- **`size`** is the text's UTF-8 length, the bytes written; the text is
  written as sent, created or replaced with one `std::fs::write`.
- **The cache.** A write goes through `Interp::file_written_as(path, id)`
  (`file_written(path)` where no id is held): it bumps the generation,
  clears every lookup, those that found nothing included, so a file just
  made shadows a builtin or a file later on the path at the next call,
  and, when any parse is cached, drops each parse whose `FileId` may name
  the file written. A `FileId` is the file's canonical path and its path
  normalised by its components; a file that cannot be canonicalised,
  deleted or a link that leads nowhere, takes its folder's canonical path
  joined to its name, and one whose folder cannot be canonicalised either
  has the normalised path alone. Two ids match when their normalised
  paths are equal or their canonical paths are, so a doubt costs a reread,
  never a stale parse. A parse's id is taken once, when the file is
  parsed, never at a reuse; a write canonicalises once, and not at all
  when nothing is cached; the match compares paths already held, so a
  write asks the file system nothing per cached parse. The callers, each
  naming the full path it wrote: `fopen` for writing, after the open so
  the file exists, whose id is kept on the `OpenFile` and reused by
  `fclose` after a write, so the pair canonicalises once; `fclose` and
  `fclose('all')`, for each file written; the whole-file writer
  `io::write_file`, which `writematrix`, `csvwrite`, `save` in every form,
  `saveas` and `print` share; `delete`, whose id is taken before the file
  is removed, so it is the file's own canonical path; and the protocol's
  `write_file`, with `files::Written::file`, the canonical path of a file
  replaced or the canonical folder joined to a new file's name. No caller
  is left that cannot name its file, so the drop-everything
  `files_changed` is gone. `fprintf` and `fwrite` to a file call nothing,
  as before; the `fopen` before them and the `fclose` after do. Every
  other parse is kept and judged at its next use by its modification time
  and length, as after any bump. Timed on Windows, release builds, the
  medians of two sets of alternating runs: 300 passes of
  `fopen('log.txt','w')`, `fprintf`, `fclose` and `s = helper(s)` with a
  118 KB `helper.m` of 2,540 arithmetic lines, 1.17 and 1.20 s at 6452d77
  and 1.24 and 1.28 s now (the first build: 20.5 s at review, with the
  review's lighter helper at 0.41 s before the cycle); 2000 passes with a
  3 KB helper, 1.91 and 2.13 s at 6452d77 and 2.13 and 2.48 s now. The
  gap is the one canonicalisation a pass, about 90 µs here. Two hard
  links to one file have two canonical paths: a write through one to a
  function file read through the other is seen through the file's stamp
  alone, as a change by another program is.
- **Acceptance item 11's `files_changed_drops_every_parse`** is now
  `a_write_drops_the_parse_of_the_file_it_wrote`, since `files_changed`
  is gone: it still rewrites a function file with the same length and
  puts its modification time back, and now writes it through its
  canonical path, a spelling other than the one it was read through.
  Beside it, `a_write_keeps_every_other_files_parse` (a helper's parse
  the same `Rc` after a loop of `fopen`, `fprintf`, `fclose`, `csvwrite`,
  `save` and `delete` of other files),
  `a_file_a_write_makes_shadows_a_builtin_at_the_next_call` and
  `a_deleted_function_file_made_again_is_read_again`.
- **The page.** The editor region is a section of its own above the
  command window in a `#middle` grid whose first row is `--editor`, half
  the column on the first wide layout, and the fourth splitter
  (`#split-mid`, horizontal) moves it by pointer and by Up and Down, 16 px
  a press, 120 px at least, as U2's do; in one column `#middle` is
  `display: contents`, so the order is the command window, the editor, the
  workspace, the history and the file browser. The tab bar lists a button
  per tab (the name, ` •` while its text differs from what was last read
  or written, compared on every change) and a close button `×`; Save,
  Run and Run Selection are disabled with no tab, New never. New takes the
  lowest of `untitled.m`, `untitled2.m`, ... that no open tab is named. The
  gutter is one `<pre>` of numbers beside the `<textarea wrap="off">`, both
  at a line height of 20 px and a top padding of 8 px, the gutter's
  numbers rebuilt only when the line count changes and its `scrollTop` set
  from the text's on every scroll and change, with 64 px of room past its
  last line so it can scroll as far as the text does with a horizontal
  bar showing. The mark is one absolutely placed element whose row is the
  custom property `--mark-row`, set through the CSS object model. A file
  in the browser opens on a double click, and on Enter once focused; a
  file open in a tab is selected rather than read again. The file's text
  is held with LF ends (`\r\n` and a lone `\r` become `\n`, as a textarea
  holds them), and a save writes every `\n` back as `\r\n` when the file
  had any `\r\n`. Tab with no selection, or one within a line, replaces it
  with four spaces; with a selection across lines it indents each line by
  four, the mirror of Shift+Tab; a selection that ends at the start of a
  line does not take that line. Both go through
  `document.execCommand('insertText')`, falling back to `setRangeText`, so
  the browser's undo keeps working. Escape makes the next Tab or Shift+Tab
  move the focus; any other key but a modifier, or leaving the text,
  cancels it. Ctrl+S or Cmd+S, F5 and F9 are handled anywhere inside the
  editor region, and F5 and F9 have their default prevented there, so a
  browser's reload key never reloads over an editor with focus; outside
  the region they are the browser's. Ctrl+S saves even an unchanged tab;
  F5 saves first only a tab that has changes or was never saved. The
  save-as field is a row of the editor, prefilled with the listed folder
  joined to the tab's name, the name before its extension selected; Enter
  or Save writes, Escape or Cancel gives up, and a refusal leaves the
  field open with the reason above the text. A save-as over an existing
  file replaces it, as the spec's `write_file` does. The close question is
  a row of the editor naming the tab, with Save, Discard and Cancel,
  Escape being Cancel. Run's entry is `run('<path>')` with every `'` in
  the path doubled, so the history holds a line that runs. A second Run or
  Run Selection while one is in flight does nothing. Run Selection with
  nothing selected sends the cursor's whole line, and an entry of nothing
  but whitespace sends nothing. After Run an error with frames activates
  the tab of the innermost frame whose file is open in a tab, or else,
  the run file's tab at the outermost frame's line, and a frame with a
  `null` line activates its tab without moving the cursor. After Run
  Selection only the error's `line` is followed, in the tab the selection
  was in, if it is still open. A stack link opens its file and puts the
  cursor on its line without marking it; only Run and Run Selection mark
  a line. Messages from `read_file` and `write_file` show in one status
  row of the editor. `app.js` holds no `confirm(`, `prompt(` or `alert(`,
  no inline handler is in the markup, and every `eval` it makes carries
  `stack: true`: a static test in `src/http.rs`,
  `the_editor_keeps_to_the_page_and_its_text`, holds all three, beside
  U2's palette and queue tests, which pass unchanged over the new files.
- **`Sec-Fetch-Site`**: the value trimmed of spaces and tabs, as every
  header value is, and compared in any case; an empty value is not
  `same-origin`.

**Deviations.** None from Scope.

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

1. `read_file` of `prog/s2.m` → `"text":"q = 1;\nw = undefined_thing + 1;\n"`, and of `prog/./crlf.txt` → `"path":"prog/crlf.txt","text":"a\r\nb\r\n"`. Cases: read_file_text_exact, handbook_editor_example.
2. `write_file` of `scratch_a.m` with the text `disp(7)` LF `disp(8)` LF → `"size":16`; `read_file` of it → the same text; `run_file` of it → `"out":"     7\n     8\n"`; then an `eval` of `delete('scratch_a.m')`. Cases: write_read_run_round_trip.
3. A function file rewritten at once with the same length: `write_file` of `scratch_f.m` with `function scratch_f` LF `disp(1)` LF `end` LF, `run_file` → `     1`; the same with `disp(2)` → `     2`; and an `eval` of a loop that writes `scratch_g.m` with `fprintf` five times, each with `disp(k)`, calling `scratch_g` after each write → `1` to `5`, each once. Then both deleted. Cases: run_file_sees_same_length_rewrite.
4. `read_file` refusals: `..` → outside; `/etc` → malformed; `""` and `prog` → not a file; `nope.m` → not a file; `path` missing → no 'path' field. Cases: err_read_file_refused, err_read_file_not_utf8_or_too_large.
5. `write_file` refusals: `..` → outside; `nofolder/x.m` → not in a folder of the file root; `prog` → is a folder; `NUL.m`, `con`, `x.m.` and `x.m ` → not a file; `text` missing → no 'text' field; `"text":3` → 'text' must be a string; a name of 300 characters → could not be written, and a `read_file` of it after → not a file. None creates anything: a following `files` of `prog` lists exactly its three files. Cases: err_write_file_refused, err_write_file_not_written.
6. `run_file` of `prog/s1.m` → `"out":"start\n"`, the message `Unrecognized function or variable 'nosuch'.`, `"line":null`, and `"stack":[{"file":"prog/s1.m","name":"helper","line":6},{"file":"prog/s1.m","name":"s1","line":3}]`; of `prog/s2.m` → `"out":""` and the one frame `{"file":"prog/s2.m","name":"s2","line":2}`; of `prog/crlf.txt` → `Path 'prog/crlf.txt' is not a .m file.` Cases: err_run_file_stack, handbook_editor_example.
7. `run_file` after an `eval` of `cd prog` still runs `prog/s2.m` from the root (the same answer as in 6), and an `eval` of `cd ..` restores the folder. Cases: run_file_from_root_after_cd.
8. `eval` of `run('prog/s1.m')` with `"stack":true` → `"out":"start\n"`, `"line":1` and the same two frames; the same without the flag → exactly U0's form, with no `stack` key; `"stack":1` and `"stack":"yes"` → `Malformed request: 'stack' must be true or false.` Cases: err_eval_stack_flag.
9. `eval` with `"stack":true` of `x = nosuchname` → `"stack":[]`; of `f = @(n) nosuch(n); f(1)` → `"stack":[{"file":null,"name":"@(n)nosuch(n)","line":null}]`. Cases: err_eval_stack_empty_and_anonymous, handbook_editor_example.
10. `.http` cases: `POST /api` with the token and `Sec-Fetch-Site: cross-site`, `same-site` and `none` → `403 Forbidden` each, a following `disp(x)` showing none was evaluated; with `Sec-Fetch-Site: same-origin` → `200 OK`. Cases: err_sec_fetch_site_never_evaluated, err_sec_fetch_site_twice_or_before_content_type, sec_fetch_site_same_origin_accepted.
11. Unit tests: the 4 MiB bounds for reading (a file of 4,194,304 bytes read, one of 4,194,305 refused without reading it) and for writing (a text of 4,194,304 bytes written, one byte more refused); a file that is not UTF-8 refused; on Unix, a dangling link and a link out of the root refused by `write_file` with nothing created; every device name and trailing character refused; the parse of a written file dropped, and only that one. Tests: `read_file_answers_the_text_exactly_under_its_normalised_path`, `read_file_refuses_what_is_not_a_file_inside_the_root`, `read_file_reads_four_mebibytes_and_refuses_one_byte_more`, `a_read_is_judged_by_its_length_first_and_bounded_after`, `read_file_refuses_text_that_is_not_utf8`, `write_file_creates_and_replaces_under_its_normalised_path`, `write_file_refuses_in_the_spec_order_and_creates_nothing`, `every_device_name_and_trailing_character_is_refused`, `write_file_writes_four_mebibytes_and_refuses_one_byte_more`, `write_file_never_follows_a_link_out_of_the_root` (Unix, and Windows where a link can be made), `run_file_needs_a_dot_m_file_inside_the_root`, `run_file_runs_a_plain_path` and `relative_names_a_frame_file_under_the_root` in `src/files.rs`; `a_write_drops_the_parse_of_the_file_it_wrote` and `run_file_runs_a_file_as_an_entry` in `src/interp.rs`; `read_file_answers_its_keys_in_order`, `write_file_answers_its_keys_in_order_and_the_next_call_reads_it`, `run_file_answers_as_eval_with_a_stack`, `eval_with_stack_adds_the_frames_after_line` and `the_stack_names_a_path_function_relative_to_the_root` in `src/protocol.rs`; `sec_fetch_site_when_present_must_be_same_origin` and `the_editor_keeps_to_the_page_and_its_text` in `src/http.rs`; `the_editor_messages` in `src/error.rs`.
12. `tests/ui_server.rs`: a `write_file`, `read_file` and `run_file` round trip over the socket in a fixture folder the test makes, and a `Sec-Fetch-Site: cross-site` request refused. Tests: `the_editor_operations_answer_over_the_socket` in `tests/ui_server.rs`.
13. Every existing case passes unchanged. Tests: the whole golden suite, `cargo test --test golden`, with every file under `tests/cases/` outside `U3-ui-editor/` unchanged.
14. A hand check in Chrome: open a file from the file browser; edit it (the `•` appears); save with Ctrl+S (it goes); change one digit and press F5 (the new digit runs); Run Selection with F9; an error in a local function moving the cursor to its line in the right tab; a stack link in the command window opening the file at its line; New, then save under a name; closing an unsaved tab asking in the page; the gutter keeping step with a long file; Tab, Shift+Tab and Escape then Tab. Tests: none automated, by design; the hand check at the close of the cycle.

## Status

Done (2026-09-30)
