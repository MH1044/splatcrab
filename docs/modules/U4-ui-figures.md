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

Recorded as decisions are made. Start from the
"Key designs to preserve" section of docs/ARCHITECTURE.md and record here:
which files and types changed, which invariants were preserved (column-major
storage, 1-based to 0-based conversion at the index boundary, the `end` stack,
name resolution order, the output sink), and any deviation from this spec
that was accepted deliberately.

Settled before the build:

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
  nothing drawn is, as cycle 12 writes it and as the binary gave it before
  this cycle's build, these four lines, each ending in LF:

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

Recorded during the build.

**Files and types.** Changed: `src/protocol.rs` gains `figures`,
`figure` (its bound a parameter), `figure_number`, `cwd`, the constant
`MAX_INLINE_SVG` (33,554,432) and three rows of its module table;
`src/files.rs` gains `current_folder(root, path)` and
`folder_relative(root, dir)`, and the private `folder` (the three steps of
the rule for a folder, which `list` now calls) and `under_root` (which
`relative` now calls); `src/interp.rs`'s `Interp::set_cwd` gains the
confinement and hands its judged target to the new
`Interp::enter_folder(folder)`, which `cwd` calls too, so both bump the
lookup generation alike, and `interp::normalize` builds its walk's
result instead of walking (see the Deviations below);
`src/error.rs` gains `request_figure_number`, `figure_not_open`,
`figure_too_large` and `cd_outside_root` (`'path' must be a string` is
U0's `request_not_string`); `src/http.rs`'s `CSP` gains
`img-src 'self' blob:`; `src/ui/` gains the inline figures, Tab
completion, the completion list under the prompt and the file browser on
`cwd`; `tests/ui_server.rs` gains
`figures_and_the_current_folder_answer_over_the_socket` and the policy
checked exactly. `src/main.rs`, `src/plot/` and `tests/golden.rs` are
unchanged.

**Invariants.** Column-major storage, the one-based conversion in
`eval_index_args`, the `end` stack and the name resolution order are
untouched. The output sink is untouched: `figure` reads
`Interp::figure_svg`, a string, and nothing reaches the writer but a
response. Invariant 6: a figure's render is bounded by cycle 12's
`MAX_POINTS`, and its answer by the 32 MiB bound, judged on the text's
length; `cwd` judges a path in time linear in its length, by U2's
`normalise` and one join; `cd` canonicalises its target once, whole, and
nothing is pushed onto a verbatim root: the folder `cwd` stores is the
root joined with the components in one push and put in its plain form, so
on Windows the current folder is a plain path except for a folder no
plain path can spell, which keeps its `\\?\` form (the Deviations below).
`cd`'s walk over its target's components, `interp::normalize`, builds the
walk's result in time linear in the path's length on every path, verbatim
or plain; it was quadratic from a verbatim current folder. No `unwrap` or `expect` reads anything a request or the file
system controls.

**Choices where the spec was silent.**

- **`n`.** A JSON number that is finite, at least 1 and whole; `1e400`,
  which the JSON parser reads as an infinity, and `-0` are `'n' must be a
  figure number.`, as are `[5]` and `true`; `5.0` and `0.5e1` name figure
  5. A whole number past a `u32` is no open figure. `<n>` is written as
  the JSON writer writes a number, the shortest digits that name the
  double padded with zeros, so `1e12` is `1000000000000` and `1e300` a `1`
  and 300 zeros. The checks run `n` (missing, then not a figure number),
  then open, then the bound, which counts the SVG's UTF-8 bytes and
  answers a text of exactly 32 MiB.
- **`cwd`'s `path`.** `null` is present, so it is `'path' must be a
  string.` With no root, which only a test builds, the answer is `null`
  and a path is judged on its text and then refused as outside, as `files`
  does. A current folder that was deleted, or that holds a component that
  is not Unicode, is `null` too, since it has no canonical path under the
  root to answer. The answer after a move is judged on the canonical path
  like any other, so a link inside the root is answered by its target's
  path, while the folder stored keeps the link's own name, as `cd` would.
- **`cd`'s confinement** is judged after the not-a-folder check, so a
  missing target keeps cycle 13's message wherever it would be. `..` is
  still resolved on the text first, as cycle 13's `cd` resolves it, so `cd
  link/..` stays where it is; the target is then canonicalised once and
  compared with the canonical root component by component, and a folder
  that exists but cannot be canonicalised counts as outside. `run` and
  `run_file` still move into the file's folder for the length of the run
  without `cd`'s check, and move back.
- **Entries run one at a time in the page**, each after the one before has
  shown its figures (`runEntry` chains `runOne` on the one before, as
  `call` chains requests), so an editor Run pressed while a command-window
  entry is in flight cannot change a figure before the first entry's
  snapshot is taken. Each figure is a `<figure class="plot">` holding a
  `<figcaption>` `Figure <n>` above an `<img alt="Figure <n>">`; its URL is
  revoked on `load` and on `error`, and the transcript scrolls to its end
  when an image loads. A refusal of `figures` or `figure`, or a transport
  failure, is a block in the error style where the image would be.
- **Tab in the command window.** The word is the run of letters, digits
  and `_` before the cursor, as the terminal's editor takes it; none, or
  one that starts with a digit, completes nothing. With a selection, or
  while an entry runs, Tab completes nothing. The quote rule counts `_`
  with the letters, since only a name ends with one, and a `"` always opens
  a string, which nothing transposes. The rule looks only at the character
  before the quote, where the lexer's is on tokens, so `x 'a`, with a
  space before the quote, is a transpose to the lexer and a string to the
  page: harmless for completion, and right for command syntax, where
  `cd 'tr` holds a string. The cursor's line alone is scanned, a string
  not spanning lines, and a quote doubled inside a string is one quote of
  it, read as one and written back doubled. A folder part that starts
  with `/` or holds `:` or `\` completes nothing without asking.
  The current folder is asked with `cwd` at each Tab, then its `files`
  joined with the folder part, so a Tab right after a `cd` sees the new
  folder; names match by prefix, case-sensitively, `.` files included.
  Several completions insert what they share and are always listed, even
  when that adds nothing (the terminal lists only then). An answer that
  arrives after the input changed is dropped. Shift+Tab moves the focus
  back, as the browser does; Escape makes the next Tab move it on, and any
  other key but a modifier, or leaving the input, cancels that. The list
  is a row of names under the prompt, cleared by any key but a modifier
  and when an entry runs.
- **The file browser.** It asks `cwd` and lists the folder it answers
  when the page loads, after every entry and after every save; a `null`
  answer lists the root with a note saying so, and a click there moves the
  current folder back under it. A click on a folder or on `..` sends `cwd`
  with that path and lists the answer; a refusal (a folder code deleted)
  lists the current folder again with the reason as a note. A `files`
  refusal of the folder `cwd` named, one deleted between the two
  requests, lists the root with the reason, as U2's pane fell back, so
  there is always a folder to click.

**Deviations.** One from Scope, accepted deliberately. On Windows a folder
whose name no plain path can spell, one ending in a dot or a space, or a
device name such as `con`, which only a `\\?\` path can make, has no
plain form that names it, so `cwd` into it stores its `\\?\` form, which
`pwd` then shows, where Scope says never. The file browser lists such a
folder like any other, and a click on it moves into it this way. `cd`'s
walk over a current folder in that form, or over a `\\?\` path typed to
`cd`, is linear: `interp::normalize` pushed one
component at a time, and a push onto a verbatim path rebuilds the whole
path, so a `cd` of 64,000 components from such a folder took 168 s; it
now builds the walk's result once, the walk's text byte for byte
(`interp::tests::normalize_is_the_walk`), and the same `cd` takes 0.025 s.

- **The folder stored.** `files::current_folder` is U3's `run_file`
  approach: the root joined with the normalised components in one push,
  then `plain`, the root's form without `\\?\` (or `\\?\UNC\`) when that
  canonicalises to the same folder, and the verbatim form otherwise, as a
  run file is named. It is the canonical root's own spelling, which may
  differ from the `pwd` a session starts with: on Unix when the working
  directory is reached through a link (`/var` and `/private/var` on
  macOS), and on Windows when it is spelled through a `subst` or mapped
  drive, an 8.3 name or a letter case the folder's names do not have,
  each of which changes to the canonical root's plain form after the
  first `cwd`; `cd` below it keeps that spelling.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/U4-ui-figures/`, as a `.proto` case unless it says otherwise.
Expected output follows from this spec's rules and U0's recorded bytes.

The fixture folder `tests/cases/U4-ui-figures/tree/` holds `a.txt` (the 2
bytes `ab`) and `sub/deep.txt` (the 3 bytes `abc`), with no line ends, and
`.gitattributes` marks it binary. The root is the case folder, named
`U4-ui-figures`.

1. `figures` at the start → `{"id":1,"ok":true,"open":[],"changed":[]}`; after an `eval` of `figure(3);` → `"open":[3],"changed":[3]`; again → `"changed":[]`; after `plot(1:3);` → `"changed":[3]`; after `figure(5); close(3);` → `"open":[5],"changed":[5]`; after `close all` → `"open":[],"changed":[]`. Cases: figures_open_and_changed.
2. `figure` of an empty figure: after `figure(5);`, `{"op":"figure","n":5}` → `"n":5` and `"svg"` holding exactly the four lines the Design notes record, JSON-escaped. Cases: figure_empty_svg_exact.
3. `figure` refusals: no `n` → `Malformed request: no 'n' field.`; `"n":0`, `"n":-1`, `"n":1.5`, `"n":"5"` and `"n":null` → `Malformed request: 'n' must be a figure number.`; `"n":7` with no figure 7 open → `Figure 7 is not open.`; `"n":1e12` → `Figure 1000000000000 is not open.` Cases: err_figure_refused, err_figure_too_large.
4. `cwd` at the start → `"cwd":""`; `{"op":"cwd","path":"tree"}` → `"cwd":"tree"`; an `eval` of `ls` → `a.txt` and `sub` on their own lines, as cycle 13's `ls` writes a folder's names; `cwd` with `tree/sub` → `"cwd":"tree/sub"`; `cwd` with `""` → `"cwd":""`. Cases: cwd_moves_and_answers.
5. `cwd` refusals: `..` → outside the file root; `tree/a.txt` → not a folder; `/etc` → the malformed path message; `"path":3` → `Malformed request: 'path' must be a string.`; and after each refusal `cwd` still answers the folder it was in. Cases: err_cwd_refused.
6. `cd` in an `eval`: at the root `cd ..` → `Cannot CD to ..: it is outside the file root.` with `"line":1`, then `cwd` → `""`; `cd tree` then `cwd` → `"tree"`; `cd ../..` → `Cannot CD to ../..: it is outside the file root.`; `cd ..` → `cwd` `""`; `cd nope` → `Cannot CD to nope (Name is nonexistent or not a directory).` Cases: err_cd_confined_to_root, handbook_figures_example.
7. A `.m` case in script mode: `cd ..` then `cd U4-ui-figures` then `disp(1)` → `     1`: with no root set, `cd` above the case folder still works. Cases: cd_unconfined_in_script_mode.
8. Unit tests: a plotted figure's `svg` equal to `saveas`'s bytes and holding its title; the 32 MiB bound reached with a smaller bound; on Windows, `pwd` after `cwd` moves the folder has no `\\?\` prefix; on Unix, a `cd` through a link that leads out of the root refused and one that stays inside accepted; the policy constant. Tests: protocol::tests::a_plotted_figures_svg_is_what_saveas_writes, protocol::tests::the_inline_bound_holds_at_and_past_its_length, protocol::tests::pwd_after_cwd_is_the_plain_form_cd_gives, interp::tests::cd_through_a_link_is_judged_by_where_it_leads, http::tests::the_policy_admits_blob_images_and_nothing_else_new.
9. `tests/ui_server.rs`: `figures` and `figure` over the socket after a `plot`; the page's `Content-Security-Policy` header exactly `default-src 'self'; img-src 'self' blob:; frame-ancestors 'none'`. Tests: figures_and_the_current_folder_answer_over_the_socket, the_server_answers_over_a_loopback_socket.
10. Every existing case passes unchanged. Tests: golden_cases.
11. A hand check in Chrome: a `plot` appears inline under its entry; a second entry drawing into the same figure adds a second snapshot and keeps the first; a figure made by Run from the editor appears under the `run(...)` entry; `cd tree` in the command window moves the file browser, and a click on a folder there moves `pwd`; `..` is offered everywhere but at the root; Tab completes `dis` to `disp`, lists the names for `di`, and completes `cd('tr` to `cd('tree/`; Escape then Tab moves the focus on; the transcript holds no `<svg>` element, only `<img>`s. Tests: the hand check.

## Status

Done (2026-09-30)
