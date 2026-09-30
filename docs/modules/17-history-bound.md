# 17 — History bound

## Goal

The history file bounded in bytes. The history (cycle 13, shared with the
browser desktop since U2) keeps the newest 1,000 entries, but
`history::load` reads the whole file into memory before it keeps them, and
neither the file nor an entry is bounded in bytes; `history` and
`history_add` load it on every page load and every entry run, so a pasted
multi-megabyte entry, or a file another program grew, makes every answer
and every call as large and as slow as the file (the Known bugs row "The
history file is read whole, with no byte bound"). This cycle bounds one
entry and the part of the file that is read.

After this module a user can do things that were not possible before; every
bullet in Scope must be demonstrable by at least one acceptance test below.

## Scope

- **An entry is bounded.** `history::MAX_ENTRY_BYTES` is 65,536. An entry
  of more UTF-8 bytes than that is not kept: `history::remember` answers
  false for it, as it does for an entry of whitespace or a repeat of the
  newest, so the terminal's line editor neither keeps nor appends it and
  the protocol's `history_add` answers `"added":false`. The entry itself
  still runs; only the history leaves it out. An entry of exactly 65,536
  bytes is kept as before
- **The file is read within a bound.** `history::MAX_FILE_BYTES` is
  4,194,304 (4 MiB). `history::load` reads at most the last
  `MAX_FILE_BYTES` of the file: a longer file is read from that many bytes
  before its end, and the partial line the read starts in is dropped. Of
  the lines read it keeps, as today, the newest `MAX_ENTRIES` (1,000),
  oldest first, leaving out any line whose entry is longer than
  `MAX_ENTRY_BYTES`, which another program may have written. A file of no
  more than `MAX_FILE_BYTES` is read whole, exactly as today
- **The file stays bounded.** `load` rewrites the file with the entries it
  kept when the file is longer than `MAX_FILE_BYTES`, as it already does
  when the file holds more than twice `MAX_ENTRIES` entries; a failure to
  rewrite it is ignored, as today. So `history` answers at most the entries
  of 4 MiB of file, and each `history_add` reads at most that
- **Nothing else changes.** The file's format, its place
  (`SPLATCRAB_HISTORY` or the home folder), the escapes, `remember`'s other
  rules, the `history` and `history_add` answers' keys and the refusal
  `The history file could not be written.` are today's. No message is
  added
- **The docs.** The Known bugs row "The history file is read whole, with
  no byte bound" is removed; invariant 6 in `docs/ARCHITECTURE.md` and the
  module comment of `src/history.rs` state the two bounds; `docs/FEATURES.md`
  and `docs/HANDBOOK.md`, where they describe the history, say so

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.
- Locking the file against two writers, and any change to how entries are
  appended.
- A message for an entry left out of the history: the history is a
  convenience, and `"added":false` is the answer for every entry it does
  not keep.

## Design notes

Recorded as decisions are made. Start from the
"Key designs to preserve" section of docs/ARCHITECTURE.md and record here:
which files and types changed, which invariants were preserved (column-major
storage, 1-based to 0-based conversion at the index boundary, the `end` stack,
name resolution order, the output sink), and any deviation from this spec
that was accepted deliberately.

Settled at planning:

- **Two bounds, both in bytes.** 1,000 entries of 64 KiB would still be
  64 MiB, so the entry bound alone does not bound an answer; the file bound
  does, and the entry bound keeps one paste from taking the whole of it.
- **The newest part of the file is the part read**, since the history keeps
  the newest entries; the file is compacted to what was kept, so the next
  load reads it whole again.
- **Both bounds are parameters of an inner function** of `history.rs`, so
  a unit test reaches them with small numbers rather than a 4 MiB file,
  as `files::MAX_TEXT` and the figure bound are reached.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/17-history-bound/`, as a `.proto` case (`--protocol`, one JSON
request per line, the history file the case's own) unless it says
otherwise.

1. `history_add` of `"x = 1"` → `"added":true`; of an entry of 65,537
   ASCII characters → `"added":false`; `history` → items `["x = 1"]` alone.
   Cases: `history_entry_too_long.proto`.
2. `history_add` of an entry of exactly 65,536 ASCII characters →
   `"added":true`; then `history_add` of the same entry → `"added":false`
   (a repeat of the newest, so it was kept). Cases:
   `history_entry_at_bound.proto`.
3. A seeded history file (the case's `.history` sibling) holding `a`, a
   line of 70,000 characters and `b` → `history` lists `["a", "b"]`.
   Cases: `history_seed_long_line.proto`.
4. Unit tests, with small bounds through the inner function: a file longer
   than the byte bound is read from its tail, its partial first line
   dropped, the newest entries kept and the file rewritten to them; a file
   within the bound is read whole and left as it is; an entry past the
   entry bound is not remembered, and one at the bound is; a line past the
   entry bound in the file is left out. Tests:
   `load_reads_at_most_the_tail_of_a_long_file`,
   `a_long_file_is_compacted_to_what_was_kept`,
   `an_entry_past_the_bound_is_not_remembered`,
   `a_long_line_in_the_file_is_left_out`.
5. Every existing case passes unchanged. Cases: the whole golden suite.

## Status

Planned
