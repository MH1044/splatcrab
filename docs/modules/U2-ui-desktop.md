# U2 — UI desktop

## Goal

The browser page becomes a desktop of four resizable panes: the command
window, a workspace pane, a file browser and a command history, with the
palette defined exactly once. The panes read what they show through new
protocol operations, so every byte they depend on is pinned by golden cases
as U0's are, and the server stops letting one idle connection hold the
interpreter. The standard library only: the layout, the splitters and every
operation are written here.

After this module a user can do things that were not possible before; every
bullet in Scope must be demonstrable by at least one acceptance test below.

## Scope

- **The layout.** Four panes in one window: the file browser on the left,
  the command window (U1's transcript and input) in the middle, and on the
  right the workspace above the command history. Three splitters: between
  the left pane and the middle, between the middle and the right, and
  between the workspace and the history. A splitter moves with the pointer
  and, once focused, with the arrow keys (16 px a press); no pane is dragged
  below 120 px. A splitter is a focusable element with `role="separator"`
  and its `aria-orientation`. Below 720 CSS pixels of window width the
  panes stack in one column, command window first, and the splitters are
  hidden. Plain HTML, CSS and JS with no framework and no external
  resource, since the page's policy is `default-src 'self'`: a pane's size
  is set through a CSS custom property from script (the CSS object model,
  which that policy allows), never through a `style` attribute
- **The palette defined exactly once.** Every colour of the page is a CSS
  custom property on `:root`, the light set in the `:root` rule and the
  dark set in the one `@media (prefers-color-scheme: dark)` block's `:root`
  rule. Outside those two rules `app.css` holds no hexadecimal colour, no
  colour function (`rgb`, `rgba`, `hsl`, `hsla`, `hwb`, `lab`, `lch`,
  `oklab`, `oklch`, `color`) and no CSS named colour; `transparent`,
  `currentColor` and `inherit` are allowed anywhere. `index.html` and
  `app.js` hold no colour at all. A unit test reads the embedded files and
  enforces all of it, listing every CSS named colour
- **Every request goes through one queue in `app.js`,** one at a time, in
  the order the page made them, so the panes' refreshes never race the
  command window or each other
- **The workspace pane** lists every variable, sorted by name as the
  protocol gives them, with its name, a value preview, its size written
  `2×3`, and its class. It is refreshed when the page loads and after every
  `eval`, whatever the `eval` answered
- **`workspace` with `"preview": true`** adds a `value` key after `class` to
  each variable: `{"name":"x","size":[1,1],"class":"double","value":"3"}`.
  Without the field, or with `"preview": false`, the answer is exactly U0's,
  byte for byte. A `preview` that is not `true` or `false` is
  `Malformed request: 'preview' must be true or false.` The preview is
  SplatCrab's own text, by the first rule that applies:
  1. a numeric or logical array, real or complex, that is not empty and
     has at most 10 elements: each element as `disp` prints it alone, under
     the current `format`, with its leading and trailing spaces removed; a
     single element stands alone (`3`, `1.0000 + 2.0000i`), and more are
     written in brackets, a `,` between the elements of a row and a `;`
     between rows, with no spaces added (`[1,2;3,4]`). An element is taken
     as a 1x1 value on its own, so an element of a complex array whose
     imaginary part is zero is shown real, as indexing gives it
  2. a char array with at most one row: its text in single quotes, each
     `'` in it doubled, as a char literal writes it (`'it''s'`, and `''`
     for an empty one)
  3. a function handle: its `func2str` text (`@(x)x+1`, `@sin`)
  4. anything else, empty numeric arrays, larger arrays, char arrays of
     several rows, cells, structs and `MException`s included: the size and
     class, `2×3 double`, `0×0 double`, `2×2 char`, `1×1 struct`,
     `1×1 MException`, with `complex ` before `double` for complex storage
     (`2×2 complex double`)

  A preview longer than 80 characters (Unicode scalar values) is cut to its
  first 80 and followed by `…`. Making a preview costs time proportional
  to what it shows, never to the value's size: rule 2 decodes at most the
  code units the cut can keep, and rule 1 never runs past 10 elements
- **The file browser** lists one folder of the file root at a time, folders
  first, each entry with its name and, for a file, its size. Clicking a
  folder lists it; a `..` entry, which the page adds everywhere but at the
  root, lists the parent. A path above the listing shows where it is. It is
  refreshed after every `eval`, since code can make and delete files, and
  falls back to the root when its folder is gone. It is read-only in this
  cycle: opening a file is cycle U3's editor
- **The file root.** Every client mode, `--protocol`, `--ui` and
  `--http-stdio`, fixes the root once as it starts: the process's working
  directory, canonicalised. It never changes, whatever `cd` does; the file
  browser following `cd` is cycle U4's
- **`files`**: `{"id":1,"op":"files","path":"tree/sub"}` answers
  `{"id":1,"ok":true,"root":"<name>","path":"tree/sub","entries":[{"name":"x","dir":true,"size":null},{"name":"a.txt","dir":false,"size":12}],"truncated":false}`.
  `path` is required and relative to the root, and `""` is the root
  itself. `root` is the last component of the root's path, or `/` when the
  root is the top of a file system; `path` is the folder listed, normalised
  (below). Entries come folders first, then everything else, each group
  sorted by name in byte order; `size` is a file's length in bytes and
  `null` for a folder. At most 10,000 entries are listed, the first in that
  order, and `truncated` says whether any were left out; the bound is a
  parameter of the listing function, so a unit test reaches it. An entry
  whose name is not valid Unicode is left out, since the page could not
  name it back
- **The confinement rule for `path`**, judged in this order before anything
  on disk is read:
  1. the path is split at `/`. A path that starts with `/`, or holds a `\`,
     a `:` or a NUL anywhere, is `Malformed request: 'path' must be a
     relative path with '/' separators.`: no absolute path, drive, UNC
     name, Windows separator or alternate data stream is ever joined to the
     root
  2. empty components and `.` are dropped and `..` removes the component
     before it; a `..` with nothing before it is `Path '<path>' is outside
     the file root.`, so `..` at the root, `tree/../..` and
     `../<root>/tree` are all refused, even when the last would come back
     inside
  3. the normalised path is joined to the root and canonicalised, which
     resolves every symbolic link and junction. A path that cannot be
     canonicalised, because it or a link on the way does not exist, is
     `Path '<path>' is not a folder.`. A canonical result that is not the
     root or inside it, compared component by component, is `Path
     '<path>' is outside the file root.`, judged before its kind, so a
     refusal says nothing about what lies outside. A result inside the
     root that is not a folder is `Path '<path>' is not a folder.`

  `<path>` is the path as the request sent it. An entry that is a symbolic
  link is listed by what it points to when that is inside the root, and
  otherwise, dangling links included, with `dir` false and `size` null, so
  a listing says nothing about anything outside the root
- **The command history pane** shows the shared history, oldest at the
  top, loaded when the page loads. Clicking an entry puts it in the input;
  double-clicking runs it. Every entry the command window runs is added,
  by the rule of `history::remember`, and Up and Down in the input walk
  this same list, so an entry typed in the terminal can be recalled in the
  page. The page stores whole entries: a multi-line entry is one entry,
  which the file's format keeps on one line
- **`history`**: `{"id":2,"op":"history"}` answers
  `{"id":2,"ok":true,"items":["x = 1","disp(x)"]}`, the entries of the
  history file (`history::load`, so at most 1000, oldest first), the same
  file the terminal's line editor reads: `SPLATCRAB_HISTORY`, else
  `.splatcrab_history` in the home folder. With no file, or no path at all,
  `items` is empty
- **`history_add`**: `{"id":3,"op":"history_add","entry":"x = 1"}` answers
  `{"id":3,"ok":true,"added":true}`. It loads the file, applies
  `history::remember`, and appends the entry with `history::append` when
  that says it is new; `added` is false for an entry that is only
  whitespace or repeats the newest one, and when there is no history path.
  `entry` is required, a string. A file that cannot be written is
  `The history file could not be written.`, with no operating-system text,
  so the answer does not depend on the platform
- **No golden case touches the user's history.** The harness gives every
  case it spawns, of every kind, `SPLATCRAB_HISTORY` pointing at a fresh
  file of its own under the temporary folder, removed before and after the
  run; when the case has a sibling `<name>.history`, its bytes are copied
  there first, so a case can start from a known history. Documented in
  `docs/TESTING.md` and at the top of `tests/golden.rs`
- **The server reads each connection on a thread of its own,** at most 16
  at once; a connection past that is closed at once, without an answer.
  Each thread reads one request under U1's limits and 10-second deadline
  and hands it, whole, to the interpreter thread, which answers requests
  one at a time in the order they arrived whole, then the connection's own
  thread writes the answer under its deadline, lingers and closes. So an
  idle or trickling connection holds only its own thread, never the
  interpreter, and a request's linger no longer delays the next. This
  closes the gap U1's Design notes recorded, which a page making several
  requests per entry would widen. `http.rs` is unchanged by it and stays
  pure: every new operation is a `POST /api` body, so there is no new route
- The REPL, script mode, `--protocol` and `--http-stdio` behave exactly as
  before for every request U0 and U1 defined, and every existing case
  passes unchanged

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.
- The editor, opening or writing a file, and running a file from the file
  browser: cycle U3. Inline figures, Tab completion in the page and the file
  browser following `cd`: cycle U4.
- Keeping pane sizes across runs. The origin changes with the port, so
  storage kept by the browser would rarely be found again.
- Editing or deleting history entries from the page, or clearing the
  history.
- A value preview for N-D arrays, which do not exist yet.
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

- **`files` is relative to the root, not to `Interp::cwd`.** The plan had it
  relative to the current folder. That would make the file browser follow
  `cd` already, which is U4's feature, and would make every listing a
  refusal once `cd` had left the root, which nothing in U2 stops. Relative
  to the root, the pane is independent of `cd` in U2, and U4 adds the
  current folder to it on purpose.
- **The preview rides on `workspace`, behind a flag,** rather than a
  `preview` operation per variable, so the pane refreshes in one request,
  and U0's bytes are untouched because the field is new and optional.
- **The fourth pane is the command history,** because cycle 13 fixed the
  history file's format for the interface to share and left "a protocol
  operation, which is the interface's cycle" to read and write it. The
  editor arrives as a fifth region in U3.
- **The lexical rule comes before the disk.** Refusing a `..` past the root
  before anything is canonicalised means no request can make the server
  touch a path outside the root, even to find it missing; the canonical
  check after it catches what the text cannot show, links and junctions.
  The window between the check and the listing is not defended: changing a
  link inside the root in that moment needs write access to the root,
  which is outside the threat model (a web page, not a local user).
- **The token now guards the file system and the history too.** `files`
  reads folder listings under the root, and `history` returns everything the
  user has typed in any session, terminal included. Both are behind the
  same token, `Host` and `Origin` checks as `eval`, which could already read
  any file through `fileread`; neither check may be relaxed.
- **Reader threads, bounded.** Sixteen idle connections can still hold every
  slot for ten seconds at a time, so a page that finds the port can deny
  service as it could in U1, but it can no longer do so with one
  connection, and the page's own requests, one at a time, never contend.
  The interpreter stays on its one thread: requests reach it through a
  channel, and it is never shared.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/U2-ui-desktop/`, as a `.proto` case unless it says otherwise.
Expected output follows from this spec's rules and U0's recorded bytes.

The fixture folder `tests/cases/U2-ui-desktop/tree/` holds exactly:
`Z.txt` (the 2 bytes `xy`), `a.txt` (empty), `b.txt` (the 5 bytes `hello`),
`sub/deep.txt` (the 3 bytes `abc`) and `emptyish/.keep` (empty), none with a
line end, and `.gitattributes` marks `tree/` binary so no checkout changes a
byte. Every case runs with the case folder as its root, so `root` is
`U2-ui-desktop`. No case lists the root itself, whose entries change
whenever a case is added.

1. `files` of `tree` → `{"id":1,"ok":true,"root":"U2-ui-desktop","path":"tree","entries":[{"name":"emptyish","dir":true,"size":null},{"name":"sub","dir":true,"size":null},{"name":"Z.txt","dir":false,"size":2},{"name":"a.txt","dir":false,"size":0},{"name":"b.txt","dir":false,"size":5}],"truncated":false}`: folders first, byte order within each group (`Z` before `a`), sizes in bytes.
2. `files` of `tree/./sub/../sub/` and of `tree//sub` → `path` `tree/sub`, one entry `deep.txt` of size 3; `files` of `tree/sub/..` → the listing of `tree`.
3. `files` of `..`, `tree/../..` and `../U2-ui-desktop/tree` → each `Path '<as sent>' is outside the file root.` with `"line":null`, and a following `files` of `tree` still answers: a refusal is an answer.
4. `files` of `/etc`, `C:/Windows`, `tree\sub` (JSON `"tree\\sub"`) and a path holding a `:` → each `Malformed request: 'path' must be a relative path with '/' separators.`
5. `files` of `tree/a.txt` and of `nope` → `Path 'tree/a.txt' is not a folder.` and `Path 'nope' is not a folder.`
6. `files` with no `path`, and with `"path":3` → `Malformed request: no 'path' field.` and `Malformed request: 'path' must be a string.`
7. After an `eval` of `x = 3; v = [1 2 3]; m = [1 2; 3 4]; s = 'it''s'; e = []; big = 1:11; z = 1+2i; zz = [1+2i 3]; t = true; c = {1}; f = @(x) x+1; st.a = 1; ch = ['ab'; 'cd'];`, `workspace` with `"preview":true` → the variables in byte order of name with `value`s `1×11 double` (big), `1×1 cell` (c), `2×2 char` (ch), `0×0 double` (e), `@(x)x+1` (f), `[1,2;3,4]` (m), `'it''s'` (s), `1×1 struct` (st), `1` (t), `[1,2,3]` (v), `3` (x), `1.0000 + 2.0000i` (z), `[1.0000 + 2.0000i,3]` (zz).
8. The bounds: `v10 = 1:10` → `[1,2,3,4,5,6,7,8,9,10]`; `w = repmat('a', 1, 100)` → `'` then 79 `a` then `…`; `zc = [1+2i 3; 4 5; 6 7]` → `3×2 complex double`.
9. `workspace` with no `preview` and with `"preview":false` → exactly U0's answer; `"preview":1` and `"preview":"yes"` → `Malformed request: 'preview' must be true or false.`
10. With no seed file: `history` → `{"id":1,"ok":true,"items":[]}`; `history_add` of `x = 1` → `"added":true`; the same again → `"added":false`; of `   ` → `"added":false`; of `a` then a line feed then `b` → `"added":true`; `history` → `["x = 1","a\nb"]`.
11. With a sibling `.history` seed holding the three lines `disp(1)`, `a\\b` and `for k = 1:2\ndisp(k)\nend` (the file's own escapes), `history` → `["disp(1)","a\\b","for k = 1:2\ndisp(k)\nend"]` as JSON writes them: the entries decoded, the second a single backslash between `a` and `b`, the third three lines.
12. `history_add` with no `entry`, and with `"entry":[]` → `Malformed request: no 'entry' field.` and `Malformed request: 'entry' must be a string.`
13. An `.http` case: `POST /api` carrying `{"id":1,"op":"files","path":"tree/sub"}` with the token → `200 OK` whose body is the protocol's answer, with a `Content-Length` counted from that body's bytes: the new operations pass through U1 unchanged.
14. `tests/ui_server.rs` (an integration test, not a golden case): over a real socket, a connection opened and left idle does not stop an `eval` on a second connection from being answered within 3 seconds; `files` and `workspace` with `preview` answer as in 1 and 7 for a fixture made by the test; the page, script and stylesheet are still served.
15. Unit tests: the palette rule over the embedded `app.css`, `index.html` and `app.js`, with a deliberately bad stylesheet refused; the confinement rule's every step, including, on Unix, a symbolic link inside the root that points outside it (refused as a path, and listed with `dir` false and `size` null) and one that points inside (listed as its target); the 10,000-entry bound reached with a smaller bound; each preview rule, the cut at 80, and a 2^20-element char row previewed without decoding it whole; `history_add` against a path that is a folder, answering `The history file could not be written.`; the server's connection bound.
16. Every existing `.proto`, `.http`, `.repl` and `.m` case passes unchanged.
17. A hand check in Chrome, as U1's: the four panes render in light and dark, each splitter moves by pointer and by arrow keys, the workspace updates after an `eval`, the file browser lists `tree`, walks into `sub` and back, and never offers `..` at the root, the history pane recalls an entry typed in the terminal, and the page stacks in one column when narrow.

## Status

Planned
