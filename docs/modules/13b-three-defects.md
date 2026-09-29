# 13b — Three defects

## Goal

Close the three Known bugs rows that are real defects rather than open
questions, before the project hands over: an element-wise operation on an
empty array with a huge dimension hangs; the memory of a cell or struct array
is not bounded, because the element cap counts elements and not bytes; and a
UTF-16 source file is unread. A bug-fix pass in the manner of cycles 01c to
01e, asked for by the project's owner on 2026-09-29.

After this module a user can do things that were not possible before; every
bullet in Scope must be demonstrable by at least one acceptance test below.

## Scope

- **An empty result costs nothing.** `x = zeros(0, 1e12); x + 1` does not
  return today: the broadcast loop in `Matrix::try_zip` runs once per column
  even when there are no rows, and cycle 10's complex paths copied the
  pattern (`x + 1i`, `x .* 1i`, `x == 1i`, `power(x, 0.5)`,
  `complex(x, x)`). Every element-wise kernel, real and complex, and every
  loop over a dimension of an operand (reductions, cumulative sums,
  comparisons, the element-wise builtins), does work proportional to the
  number of elements it produces or reads, never to a dimension of an empty
  operand. Grep for every loop over `rows` or `cols` and judge each
- **Cells and structs are bounded by bytes, not only by elements.** A
  double costs 8 bytes and `check_shape`'s element cap of 2^28 bounds a
  double array at 2 GB; a cell element or a struct element costs many times
  that, so `s(2^27).a = 1` or `cell(1, 2^27)` can ask for tens of
  gigabytes and end in an allocator abort. Every place that makes or grows a
  cell or struct array (the constructors, indexed growth, concatenation,
  `repmat`, `num2cell`, `struct(...)`, `load`, `cellfun` and `arrayfun`
  with `UniformOutput` false, and deal) judges the array's size in bytes
  against the same 2 GB budget a double array has, counting a cell element
  as `size_of::<Value>()` and a struct element as that times its field
  count plus one, and refuses past it with `check_shape`'s own message form.
  Arrays within the budget behave exactly as before
- **UTF-16 source files are read.** A script or function file that starts
  with a UTF-16 byte-order mark (`FF FE` little-endian, `FE FF` big-endian)
  is decoded as UTF-16; so is one without a mark whose first bytes show the
  UTF-16 pattern of ASCII text, a zero byte in every odd (little-endian) or
  every even (big-endian) position of its first 64 bytes. Everything else
  is read as today: UTF-8, with a leading UTF-8 byte-order mark skipped and
  invalid bytes decoded leniently. The same decoding serves `fileread`,
  since it reads source text too. The existing case
  `01e-display-and-parser/err_utf16_file`, which pinned the defect, runs
  instead of failing, and changes for that reason

## Out of scope

- Anything not listed in Scope.
- The other Known bugs rows, which are open questions marked verify first or
  are features no cycle claims (N-D arrays, hex literals).
- Other encodings (UTF-32, legacy code pages beyond the lenient decode).

## Design notes

- **Empty results.** The broadcast kernels `Matrix::try_zip` and
  `Matrix::zip_c` loop once per element produced, `k` from `0` to
  `rows * cols`, with `(k % rows, k / rows)` as the position, instead of a
  column loop around a row loop; `transpose` maps each output element to
  its source the same way. Where a nested loop is kept, an empty operand
  returns at once: `math::scan` (`cumsum`, `cumprod`), `fft` and `ifft`,
  `sort`, `kron`, `repmat`, `fliplr`, `flipud`, `triu`, `tril`,
  `numerics::map_slices` and vertical concatenation of matrices, cells and
  structs. The grep also found the index resolvers: `x(:, :)` of a 0x1e12
  `x` listed 1e12 column positions and `x(:, 5) = []` built a 1e12-byte
  mask, both allocator aborts, so `resolve_read` and `resolve_write` return
  no positions for an empty selection, `resolve_delete` works out an empty
  array's new size from the deleted subscripts alone, and `Sel::covers`
  answers false without a mask when there are fewer subscripts than
  positions. Loops that were judged and left: `for` over the columns of an
  array with no rows (its own verify-first Known bugs row), `csvwrite` and
  `save -ascii` of an n-by-0 matrix, which write one line per row as before
  (output produced), and display and `mat2str`, which already return early
  for an empty. The unit test builds a 0x2^40 and a 2^40x0 matrix, which
  a debug build could not get through before.
- **The byte budget.** `args::MAX_BYTES` is `MAX_ELEMS * 8`, 2^28 x 8 =
  2^31 bytes. `args::CELL_UNIT` is `size_of::<Value>()` and
  `args::struct_unit(f)` is `CELL_UNIT * (f + 1)`, saturating.
  `args::check_bytes(rows, cols, unit)` runs `check_shape` first, so the
  element cap and its message are unchanged, and then refuses a product
  past `MAX_BYTES` with the same `size_overflow` message naming the size
  asked for; `check_cell` and `check_struct` are its two forms. At the
  current `size_of::<Value>()` of 72 bytes (64-bit) a cell may have at most
  2^31 / 72 = 29,826,161 elements and a one-field struct array
  2^31 / 144 = 14,913,080, so `cell(1, 2^27)` and `s(2^27).a = 1` both
  report `Requested 1x134217728 array exceeds the maximum array size.`
  The checks sit before each allocation: `cell`, `struct(...)` (after the
  field count and the cell dimensions are known), `num2cell`, the
  `UniformOutput` false branch of `cellfun` and `arrayfun`, the cell
  literal, the `c(idx)` and `s(idx)` reads, brace and paren growth of a
  cell, paren growth of a struct array and `s(k).f` growth, which counts a
  new field (so adding a field to a large array is judged too), cell and
  struct concatenation after the joined shape is known, and `load`. In
  `load` the struct check follows the existing truncation test and the cell
  check applies only to a cell the file's bytes could hold, so a hostile
  header still reports the corrupt or truncated file it is. `repmat`
  refuses a cell outright and `deal` builds no array of its own (its
  targets grow through the growth checks), so neither needs one. The
  `UniformOutput` true path of `arrayfun` still gathers its results as
  values before building the matrix; it is not a cell or struct array and
  is left as it was.
- **UTF-16.** `lexer::decode_source(bytes)` is the one decoder, used by the
  script runner in `main.rs`, `Interp`'s file loader (functions, scripts
  and `run`), `help` of a file and `fileread`. A mark `FF FE` or `FE FF` is
  dropped and the rest decoded; without a mark, the first
  `min(len, 64)` bytes rounded down to even must show zeros at exactly the
  odd positions (little-endian) or exactly the even ones (big-endian): a
  zero in every such position and none in the others, which is the pattern
  of ASCII text and which a UTF-8 file with a stray NUL, or all NULs, does
  not show. `char::decode_utf16` decodes, with a lone surrogate and an odd
  trailing byte as U+FFFD, so an odd-length file runs up to the stray byte
  and then meets the ordinary `unexpected character` error if it is not in
  a comment. Anything else is `String::from_utf8_lossy` as before, the
  UTF-8 mark left for the lexer to skip (and `fileread` still returns it,
  as it did). `01e-display-and-parser/err_utf16_file`, a BOM-less
  little-endian file, now prints `     1` and exits 0: its `.out` changes
  and its `.err` goes, for this pass.

**Fixed at review.** Two builtins made a cell from a char subject without
judging its bytes: `strsplit`, one element per piece, and `regexp`'s
`'match'`, `'tokens'`, `'tokenExtents'` and `'split'` outputs, one per
match, with `'names'` a struct array of the same length. So
`strsplit(repmat(',', 1, 3e7), ',', 'CollapseDelimiters', false)` built a
3e7-element cell past the budget, and a subject at the element cap would
have asked for about 19 GB. `strsplit` now counts its pieces in a first
pass and judges them before copying any (`err_strsplit_over_byte_budget`),
and `regexp` judges every cell and struct output against the match count
before building it. The two byte-budget cases pin the whole message,
`Requested 1x134217728 array exceeds the maximum array size.`, rather than
its first word. Out of reach of the budget, as before, a UTF-16 file with
no mark whose first 64 bytes hold a character past U+00FF, which is read
as UTF-8.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/13b-three-defects/`. Expected output comes from this spec's rules
and outputs existing cases already pin. Every new error message needs an
`err_*` case.

1. `x = zeros(0, 1e12); tic; y = x + 1; z = x .* 1i; w = x == 1i; v = power(x, 0.5); disp(isempty(y) && isempty(z) && isempty(w) && isempty(v)); disp(toc < 2)` → `   1\n   1`. Cases: empty_wide_elementwise.
2. `x = zeros(1e12, 0); tic; y = x'; z = -x; disp(isempty(y) && isempty(z)); disp(toc < 2)` → `   1\n   1`. Cases: empty_tall_transpose_negate.
3. `s(2^27).a = 1` → a clean error, exit 1, in `check_shape`'s message form (pinned in the case once the implementation settles the size it names). Cases: err_struct_growth_over_byte_budget.
4. `c = cell(1, 2^27)` → a clean error, exit 1, the same form. Cases: err_cell_over_byte_budget.
5. `s(1000).a = 1; disp(numel(s)); c = cell(1, 1000); disp(numel(c))` → `        1000\n        1000`: arrays within the budget are unchanged. Cases: cell_struct_within_budget.
6. A UTF-16LE file with a byte-order mark and one without, and a UTF-16BE file with one, each holding `disp(7)`, written by the case with `fwrite` and run with `run` (cycle 13) → `     7` three times; each file is deleted by the case. Cases: utf16_source_files.
7. `01e-display-and-parser/err_utf16_file` runs and prints what its script says, instead of the unexpected-character error: its `.out` and `.err` change, and the commit names this pass as the reason

## Status

Done (2026-09-29)
