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

Recorded during implementation:

- **Files and types.** `src/history.rs` alone in the product code, and one
  doc comment of `src/term.rs`, whose `read_line` names the new rule. Two
  public constants stand beside `MAX_ENTRIES`: `MAX_ENTRY_BYTES`, a
  `usize` of 65,536, and `MAX_FILE_BYTES`, a `u64` of 4,194,304, a `u64`
  as `files::MAX_TEXT` is, since it is compared with a file's length.
  Three private functions are new: `load_within(path, max_bytes,
  max_entry)`, which `load` calls with the two constants;
  `remember_within(entries, entry, max_entry)`, which `remember` calls
  with `MAX_ENTRY_BYTES`; and `read_tail(path, max_bytes)`, which does the
  reading. `entry_lines`, the lines of a text that hold an entry without
  their LF or CR LF, is shared by `load_within` and `parse`, whose answer
  is unchanged. `Editor::remember` and the protocol's `history_add` call
  `remember` as before, so both leave a long entry out with no change of
  their own; no type changed and no message was added.
- **The entry bound is judged first.** `remember_within` refuses an entry
  of more than `max_entry` bytes before it trims the entry or compares it
  with the newest, so a long entry is never scanned; its length is the
  byte length of its UTF-8, so `"é"` counts two.
- **How the tail is read.** `read_tail` opens the file and takes its
  length from the metadata. A file of no more than `max_bytes` is read
  whole. Past that it seeks to one byte before the last `max_bytes` and
  reads that byte and the tail; everything up to and including the first
  LF among them is dropped. So a read that starts at the first byte of a
  line keeps that line, the byte before being its LF; one that starts at
  the CR or the LF of a CR LF drops only the rest of that line ending; and
  one that starts inside a multi-byte character drops only the partial
  line, since an LF never occurs inside a UTF-8 character, and no
  replacement character is made. The bytes kept are then decoded
  leniently, as before. Both reads go through `Read::take`, so a file
  that grows between the measure and the read, or a device that reports
  a length of 0, is read no further than the bound. A file that another
  program cut short between the measure and the read, compacted by a
  second front end say, gives fewer bytes than were asked for; `load`
  answers what it read but does not count the file as longer, so it never
  rewrites a history from a read that found almost nothing. Two writers
  are otherwise out of scope, as before.
- **A long line in the file is judged after decoding and left out before
  the newest entries are counted.** The Scope says of the lines read that
  `load` keeps the newest `MAX_ENTRIES`, "leaving out any line whose entry
  is longer than `MAX_ENTRY_BYTES`"; the two orders differ only when a
  long line sits among the newest thousand, and it was decided that such a
  line takes no place among them, so the history holds a full thousand
  entries when the lines read hold as many, as it would had `remember`
  refused the line in the first place. The lines are walked newest first,
  so only the lines kept, and any long ones among them, are decoded. An
  escape shrinks as it decodes and never grows, so no line of UTF-8 of at
  most `MAX_ENTRY_BYTES` bytes is ever left out (a line that is not UTF-8:
  see Settled in testing).
- **When the file is rewritten.** When it was longer than `max_bytes`, or
  when the lines read hold more than twice `MAX_ENTRIES` entries, counting
  every line that is not empty, a long one included, as the count did
  before. A file within both bounds is left byte for byte as it is, a
  long line in it included (acceptance test 3's seed). A tail with
  no whole line in it, a single line of 50 MB say, keeps nothing and
  leaves an empty file. A failure to rewrite is ignored, as before; the
  unit test makes the file read-only, and where the file system lets its
  owner write a read-only file the rewrite simply succeeds.
- **Invariants preserved.** The output sink: `history.rs` writes nothing
  but the history file, and the protocol answers through its writer as
  before. Invariant 6: `load` reads at most `MAX_FILE_BYTES` and the byte
  before them, 4 MiB and one byte, whatever the file's size, and holds
  those bytes, their text and the entries it keeps, at most 1,000 of at
  most 64 KiB each; a byte that is not UTF-8 decodes to U+FFFD, three
  bytes, so the text is at most three times the bytes read, and the
  entries are no longer than the text. A `history` answer carries at most
  1,000 entries decoded from at most 4 MiB of the file, and a byte read
  becomes at most six bytes of the answer (a control character, which the
  JSON writer writes `\u00xx`), so an answer is at most 24 MiB and three
  bytes an entry past its keys, where before this cycle it grew with the
  file. The time of `load`, and so of `history`, of `history_add` and of
  the line editor's start, is linear in the bytes read.
- **In testing**, against a debug build under `--protocol`, a history
  file of 100 MB of short lines, one of a single 50 MB line (with a final
  LF and without), one of a million lines past 4 MiB and one of a million
  lines within it, and one of 100 MB of random bytes each answered
  `history`, or a first `history_add`, within half a second, the
  process's start included, with at most
  1,000 entries, and left the file compacted (to about 10 KB for the short
  lines, 10 bytes for the single line after the entry appended, under
  500 KB for the random bytes), each later call a few milliseconds.
  `history_add` answered `"added":false` for 65,537 ASCII characters, for 32,768 `é` and one more byte, and for
  8 MiB, and `"added":true` for 65,536 ASCII characters and for 32,768
  `é`.
- **No deviation from the Scope** was needed; the one reading above, the
  order of the entry bound and the newest thousand, is recorded as a
  decision.

Settled in testing:

- **A line that is not UTF-8 is judged by what it decodes to.** Its bytes
  are decoded leniently, as before, each byte that is not UTF-8 becoming
  U+FFFD, three bytes, and what is measured is the entry `history` would
  answer, as `remember` measures an entry, so no entry kept is ever longer
  than `MAX_ENTRY_BYTES`. A line of 21,845 bytes of `0xFF`, 65,535 bytes
  decoded, is kept, and one of 21,846, 65,538 decoded, is left out, though
  both lines are shorter than the bound. Only a file another program wrote
  can hold such a line. `a_long_line_in_the_file_is_left_out` holds it
  with a bound of 8.
- **A rewritten file can be longer than the lines it was read from.** The
  rewrite writes each kept entry in the file's own escaped form, so a line
  another program wrote comes back longer when it held a byte that is not
  UTF-8 (one byte becomes the three of U+FFFD) or a backslash pair that is
  not an escape (its backslash doubled): a file of a 4 MiB filler line and
  69 lines of `\q` written 30,000 times, 60,001 bytes each, answers 69
  entries and is rewritten to 6,210,069 bytes, past the bound, so the next
  load reads its tail, answers 46 and rewrites it to 4,140,046 bytes, and
  every load after answers 46. The file settles after that second load,
  since its lines are then this module's own, and every answer stays within
  the bounds; keeping only the entries whose written lines fit the bound
  would change what the Scope states, and is not done. The module comment
  of `src/history.rs` says so.

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

Done (2026-09-30)
