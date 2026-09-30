# U4 — UI figures

## Goal

Plots appear inline in the command window, Tab completes in the page as it
does in the terminal, and the file browser is the current folder: `cd` in
the command window moves it, and walking it moves the current folder. Gated
on 12 and 13, both done. The standard library only, and no figure is ever
inserted into the page as markup.

After this module a user can do things that were not possible before; every
bullet in Scope must be demonstrable by at least one acceptance test below.

## Scope

- **`figures`**: `{"id":1,"op":"figures"}` answers
  `{"id":1,"ok":true,"open":[3,5],"changed":[5]}`: the open figures'
  numbers, ascending (`Interp::figure_numbers`), and the open figures
  changed since the last `figures`, ascending
  (`Interp::take_changed_figures`, cycle 12's rule: `figure(n)` and every
  call that draws mark a figure changed, and closing one forgets it)
- **`figure`**: `{"id":2,"op":"figure","n":5}` answers
  `{"id":2,"ok":true,"n":5,"svg":"<?xml ..."}`, `Interp::figure_svg(n)`, the
  same text `saveas(n, 'f.svg')` writes. `n` is required (else the existing
  `Malformed request: no 'n' field.`) and must be a JSON number that is a
  positive whole number, else `Malformed request: 'n' must be a figure
  number.`; one that is no open figure, a number past any figure's
  included, is `Figure <n> is not open.`, `<n>` written as an integer. An
  SVG text longer than 32 MiB (33,554,432 bytes) is `Figure <n> is too
  large to show inline; save it with saveas.`, the bound a parameter of the
  function so a unit test reaches it; the render's own cost is bounded by
  cycle 12's `plot::figure::MAX_POINTS`
- **Inline figures.** After every entry the command window runs, typed,
  run from the editor with Run or Run Selection, or recalled from the
  history, the page asks `figures`, then `figure` for each changed number,
  and shows each under that entry's output in the transcript, labelled
  `Figure <n>`: a snapshot of the figure as the entry left it, so a later
  entry that changes the same figure adds a new snapshot under itself and
  the old one stays. Each is an `<img>` whose source is a `blob:` URL made
  from the SVG text with `URL.createObjectURL`, revoked once the image has
  loaded; no SVG is ever inserted into the page as markup, so no script in
  one can run, since an image runs none. A `figure` refusal (too large) is
  shown in the error style in its place
- **The page's policy** becomes `default-src 'self'; img-src 'self' blob:;
  frame-ancestors 'none'`, the one change to U1's headers: `default-src
  'self'` alone refuses a `blob:` image, and a `blob:` URL can be made only
  by the page's own script, which the policy still restricts to `/app.js`
- **Tab completion in the page.** Tab in the command window's input
  completes the word before the cursor through `completions`: one item
  replaces the word; several replace it with their longest common prefix
  and are listed under the prompt until the next key; none does nothing.
  When the cursor is inside a quoted string, by the lexer's rule that a
  quote after a letter, digit, `)`, `]`, `}`, `.` or a closing quote is a
  transpose and any other quote opens a string, Tab completes a file or
  folder name instead, from the `files` listing of the current folder
  joined with the folder part of what the string holds (a folder gets a
  trailing `/`); a folder the listing refuses completes nothing. Escape
  lets the next Tab move the focus on, as in the editor. Tab in the editor
  keeps U3's meaning
- **`cwd`**: `{"id":3,"op":"cwd"}` answers `{"id":3,"ok":true,"cwd":"tree"}`,
  `Interp::cwd` relative to the root with `/` separators, judged on its
  canonical path (`""` at the root; `null` if it were ever outside, which
  confinement prevents). `{"id":4,"op":"cwd","path":"tree/sub"}` moves the
  current folder to that folder of the root, the path judged exactly as
  `files` judges it (U2's confinement rule and messages; a file is `Path
  '<path>' is not a folder.`), and answers the new value. The folder it
  moves to is written in the root's own plain form, so `pwd` afterwards
  shows what `cd` to the same folder would, never a `\\?\` prefix. `path`,
  when present, must be a string
- **The file browser is the current folder.** It lists the `files` of
  `cwd`'s answer when the page loads and after every entry; a click on a
  folder, or on `..` (offered everywhere but at the root), sends `cwd` with
  that folder's path and lists it. `cd` in the command window therefore
  moves the file browser, and walking the file browser moves `pwd`
- **`cd` is confined to the root in every client mode.** When the file
  root is set, under `--protocol`, `--ui` and `--http-stdio`, a `cd` whose
  target is a folder outside the root, judged on canonical paths, is
  `Cannot CD to <dir>: it is outside the file root.`, `<dir>` as written;
  a target that is not a folder keeps today's message. The REPL and script
  mode set no root and `cd` there is unconfined, as today. This keeps the
  command window and the file browser on one folder; it is not a sandbox,
  since `fileread`, `run` and every file builtin still take any absolute
  path, which the Design notes record
- The REPL, script mode and every existing operation behave exactly as
  before, and every existing case passes unchanged

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.
- Interacting with a figure in the page: zoom, pan, data tips, resizing,
  saving from the page (`saveas` and `print` do that).
- A figure pane separate from the transcript; closing a figure from the
  page.
- Completing inside the editor, completing arguments or struct fields, and
  completion after a command-syntax word (`cd tr`).
- Confining any builtin but `cd`.
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

- **Separate operations again.** The eval answer does not carry the changed
  figures or the current folder, since U0's bytes are pinned; `figures`,
  `figure` and `cwd` are asked for, as `workspace` with `preview` and
  `history` are.
- **`blob:` over `data:`.** Both need naming in the policy. A `blob:` URL
  needs no base64 copy of a large SVG and is revoked once shown, and only
  the page's own script can make one. No golden case carries the page's
  headers (a `GET /` case would pin the whole page's bytes), so the policy
  is pinned by `http.rs`'s constant, its unit test and `tests/ui_server.rs`.
- **Figures in the transcript, as snapshots,** because the Goal's
  "inline rather than in floating windows that get lost" is about keeping
  the plot beside the code that made it, and a snapshot per entry keeps the
  record honest when a later entry changes the figure.
- **The empty figure's SVG** is the one figure answer a golden case pins
  whole: it holds no coordinate a roundoff could change. `figure(5)` with
  nothing drawn is, as cycle 12 writes it and as the binary gives it at
  this cycle's planning, these four lines, each ending in LF:

  ```text
  <?xml version="1.0" encoding="UTF-8"?>
  <svg xmlns="http://www.w3.org/2000/svg" version="1.1" width="560" height="420" viewBox="0 0 560 420" font-family="Helvetica, Arial, sans-serif">
  <rect class="figure" x="0" y="0" width="560" height="420" fill="#ffffff" stroke="none"/>
  </svg>
  ```

  A figure with a plot is pinned by a unit test (its `svg` equals the bytes
  `saveas` writes, and holds its title) and by `tests/ui_server.rs`.
- **`cd` confinement reveals existence.** A target that is not a folder
  keeps today's message, so `cd ../missing` and `cd ../present` differ; a
  client that holds the token learns as much from `exist` or `ls`, and the
  confinement is for the file browser's sake, not a sandbox.
- **Which `cd` is confined:** exactly when `Interp::file_root` is set,
  which every client mode does and nothing else does, so the rule needs no
  flag of its own, and a script run with `run_file` or `run` inside a
  client session is confined too.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/U4-ui-figures/`, as a `.proto` case unless it says otherwise.
Expected output follows from this spec's rules and U0's recorded bytes.

The fixture folder `tests/cases/U4-ui-figures/tree/` holds `a.txt` (the 2
bytes `ab`) and `sub/deep.txt` (the 3 bytes `abc`), with no line ends, and
`.gitattributes` marks it binary. The root is the case folder, named
`U4-ui-figures`.

1. `figures` at the start → `{"id":1,"ok":true,"open":[],"changed":[]}`; after an `eval` of `figure(3);` → `"open":[3],"changed":[3]`; again → `"changed":[]`; after `plot(1:3);` → `"changed":[3]`; after `figure(5); close(3);` → `"open":[5],"changed":[5]`; after `close all` → `"open":[],"changed":[]`.
2. `figure` of an empty figure: after `figure(5);`, `{"op":"figure","n":5}` → `"n":5` and `"svg"` holding exactly the four lines the Design notes record, JSON-escaped.
3. `figure` refusals: no `n` → `Malformed request: no 'n' field.`; `"n":0`, `"n":-1`, `"n":1.5`, `"n":"5"` and `"n":null` → `Malformed request: 'n' must be a figure number.`; `"n":7` with no figure 7 open → `Figure 7 is not open.`; `"n":1e12` → `Figure 1000000000000 is not open.`
4. `cwd` at the start → `"cwd":""`; `{"op":"cwd","path":"tree"}` → `"cwd":"tree"`; an `eval` of `ls` → `a.txt` and `sub` on their own lines, as cycle 13's `ls` writes a folder's names; `cwd` with `tree/sub` → `"cwd":"tree/sub"`; `cwd` with `""` → `"cwd":""`.
5. `cwd` refusals: `..` → outside the file root; `tree/a.txt` → not a folder; `/etc` → the malformed path message; `"path":3` → `Malformed request: 'path' must be a string.`; and after each refusal `cwd` still answers the folder it was in.
6. `cd` in an `eval`: at the root `cd ..` → `Cannot CD to ..: it is outside the file root.` with `"line":1`, then `cwd` → `""`; `cd tree` then `cwd` → `"tree"`; `cd ../..` → `Cannot CD to ../..: it is outside the file root.`; `cd ..` → `cwd` `""`; `cd nope` → `Cannot CD to nope (Name is nonexistent or not a directory).`
7. A `.m` case in script mode: `cd ..` then `cd U4-ui-figures` then `disp(1)` → `     1`: with no root set, `cd` above the case folder still works.
8. Unit tests: a plotted figure's `svg` equal to `saveas`'s bytes and holding its title; the 32 MiB bound reached with a smaller bound; on Windows, `pwd` after `cwd` moves the folder has no `\\?\` prefix; on Unix, a `cd` through a link that leads out of the root refused and one that stays inside accepted; the policy constant.
9. `tests/ui_server.rs`: `figures` and `figure` over the socket after a `plot`; the page's `Content-Security-Policy` header exactly `default-src 'self'; img-src 'self' blob:; frame-ancestors 'none'`.
10. Every existing case passes unchanged.
11. A hand check in Chrome: a `plot` appears inline under its entry; a second entry drawing into the same figure adds a second snapshot and keeps the first; a figure made by Run from the editor appears under the `run(...)` entry; `cd tree` in the command window moves the file browser, and a click on a folder there moves `pwd`; `..` is offered everywhere but at the root; Tab completes `dis` to `disp`, lists the names for `di`, and completes `cd('tr` to `cd('tree/`; Escape then Tab moves the focus on; the transcript holds no `<svg>` element, only `<img>`s.

## Status

Planned
