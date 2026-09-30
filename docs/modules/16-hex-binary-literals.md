# 16 — Hex and binary literals

## Goal

Numbers written in hexadecimal and binary. MATLAB has read `0x2A` and
`0b101010` as numbers since R2019b; SplatCrab reads `0x2A` as the number
`0` followed by the name `x2A`, which is `unexpected 'x2A'` (the Known bugs
row QA D30). This cycle teaches the lexer the literals, their optional
integer-type suffixes and their two's-complement negative values, and
stores each as the double its value is, since SplatCrab has no integer
classes.

After this module a user can do things that were not possible before; every
bullet in Scope must be demonstrable by at least one acceptance test below.

## Sources

Quoted at planning, on 2026-09-30, from the MathWorks page "Hexadecimal and
Binary Values".

- **S1, the notation.** "Starting in R2019b, you can write hexadecimal and
  binary values as literals using an appropriate prefix as notation. For
  example, `0x2A` is a literal that specifies 42—and MATLAB stores it as a
  number, _not_ as text." "Hexadecimal literals start with a `0x` or `0X`
  prefix, while binary literals start with a `0b` or `0B` prefix. MATLAB
  stores the number written with this notation as an integer." "Use `0`-`9`,
  `A`-`F`, and `a`-`f` to represent hexadecimal digits. Use `0` and `1` to
  represent binary digits."
- **S2, the type.** "By default, MATLAB stores the number as the smallest
  unsigned integer type that can accommodate it. However, you can use an
  optional suffix to specify the type of integer that stores the value."
  "To specify unsigned 8-, 16-, 32-, and 64-bit integer types, use the
  suffixes `u8`, `u16`, `u32`, and `u64`." "To specify signed 8-, 16-, 32-,
  and 64-bit integer types, use the suffixes `s8`, `s16`, `s32`, and `s64`."
- **S3, negative values.** "When you specify signed integer types, you can
  write literals that represent negative numbers. Represent negative numbers
  in two's complement form." Its examples: `A = 0x2As32` is `42`, `A =
  0xFFs8` is `-1`, `A = 0x2A` and `B = 0b101010` are `42`, and `register =
  0b10010110` is `150`, each shown with its integer class (`int32`,
  `int8`, `uint8`).
- **S4, large values.** `C = [0xFF000000001F123As64 0x1234FFFFFFFFFFFs64]`
  is the `int64` row `-72057594035891654 81997179153022975`; the page adds
  that converting the same integers through a double loses precision,
  `int64([-72057594035891654 81997179153022975])` giving
  `-72057594035891656 81997179153022976`, the values the nearest doubles
  hold.

## Scope

- **The literals.** Where a number starts, the digit `0` followed straight
  away by `x` or `X` begins a hexadecimal literal, and by `b` or `B` a binary
  one (S1). The literal is the prefix and the run of letters, digits and
  underscores after it: one or more digits of its base (`0`-`9`, `A`-`F`,
  `a`-`f` for hexadecimal; `0` and `1` for binary), then optionally one of
  the eight suffixes of S2, `u8`, `u16`, `u32`, `u64`, `s8`, `s16`, `s32`
  or `s64`, and nothing else. It is one number token: it ends a value and
  starts one as any number does, so `[0x1 0x2]` is two elements, `-0x10` is
  `-16`, `0x10'` is `16` and `v(0x2)` indexes. A number that starts with any
  other digit, or with `0` followed by anything else, lexes as today, so
  `00x1F` and `1x2` are still a number followed by a name, and a name such as
  `a0x1` is unchanged
- **The value.** With no suffix, the digits' value, which must be less than
  2^64 (S2's largest unsigned type). With `uN`, the digits' value, which
  must be less than 2^N. With `sN`, the digits are an `N`-bit pattern in
  two's complement (S3): the value must be less than 2^N, and a value of
  2^(N-1) or more stands for itself minus 2^N, so `0xFFs8` is `-1` and
  `0x80s8` is `-128`. Leading zero digits are allowed and count for nothing
- **Stored as a double.** SplatCrab has no integer classes, so a literal is
  the double nearest its value, class `double`: exact up to 2^53, and past
  it the nearest double, as S4's own conversion shows
  (`[0xFF000000001F123As64 0x1234FFFFFFFFFFFs64]` holds `-72057594035891656`
  and `81997179153022976`). MATLAB's class, the smallest unsigned integer
  type or the suffix's (S2), is recorded in Known deviations, with the
  integer arithmetic and display that come with it
- **A malformed literal** is today's lexer message for a malformed number,
  `invalid number '<text>'`, naming the literal as written: no digit after
  the prefix (`0x`, `0xu8`), a digit outside the base (`0b102`), a letter
  that begins no suffix or a suffix not among the eight (`0x1Fz`,
  `0x1Fu9`, `0x1Fi`), and a value its type cannot hold (`0x100u8`,
  `0x100s8`, `0x10000000000000000`). Like every lexer error, it stops a
  script before anything in it runs
- **The docs.** The Known bugs row "Hex and binary literals are
  unsupported (QA D30)" is removed; a Known deviations row records the
  class; `docs/HANDBOOK.md`, `README.md` and `docs/FEATURES.md` show the
  literals

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.
- Integer classes (`uint8`, `int32` and the rest), and `dec2hex`, `dec2bin`,
  `hex2dec`, `bin2dec`, `bitand`, `bitshift` and `bitset`.
- Underscores or other separators between digits, which S1 does not allow.

## Design notes

Recorded as decisions are made. Start from the
"Key designs to preserve" section of docs/ARCHITECTURE.md and record here:
which files and types changed, which invariants were preserved (column-major
storage, 1-based to 0-based conversion at the index boundary, the `end` stack,
name resolution order, the output sink), and any deviation from this spec
that was accepted deliberately.

Settled at planning:

- **A literal is a lexer matter**, as imaginary literals are (cycle 10):
  the lexer reads it whole and emits one `Token::Num`, so the parser, the
  command-syntax judgement and every consumer of a number are unchanged.
- **The value, not the digit count, decides whether it fits**, so leading
  zeros are free, as S2's "can accommodate it" reads.
- **A double, not an integer class.** Building integer classes is far larger
  than this row; the double holds every value S3's examples show exactly,
  and S4's own conversion gives the values past 2^53.
- **No new message.** A malformed literal is a malformed number.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/16-hex-binary-literals/`, as a `.m` case unless it says
otherwise. Values past what `disp` shows in full are printed with
`fprintf('%d\n', ...)`.

1. The values of S1 to S3: `disp(0x2A)`, `disp(0X2a)`, `disp(0b101010)` and
   `disp(0B101010)` → `    42` each; `disp(0x2As32)` → `    42`;
   `disp(0xFFs8)` → `    -1`; `disp(0b10010110)` → `   150`;
   `disp(0xFFu8)` → `   255`; `disp(0x7Fs8)` → `   127`; `disp(0x80s8)`
   → `   -128`; `disp(0xFFFFs16)` → `    -1`; `disp(class(0x2A))` →
   `double`; `disp(0x000000000000000000FF)` → `   255`. Cases:
   `hex_binary_values.m`.
2. Large values: `fprintf('%d\n', 0xFFFFFFFFs32)` → `-1`;
   `fprintf('%d\n', 0xFFFFFFFFu32)` → `4294967295`; `fprintf('%d\n',
   0xFFFFFFFFFFFFFFFFs64)` → `-1`; `fprintf('%d %d\n',
   [0xFF000000001F123As64 0x1234FFFFFFFFFFFs64])` → `-72057594035891656
   81997179153022976`; `disp(0x20000000000000 == 2^53)` → `   1`. Cases:
   `hex_binary_large.m`.
3. In expressions: `x = [0x1 0x2 0b11]; disp(x)` → `     1     2     3`;
   `disp(-0x10)` → `   -16`; `disp(0x10')` → `    16`; `disp(0x10 +
   0b1)` → `    17`; `v = 10:10:50; disp(v(0x2))` → `    20`; `a0x1 = 5;
   disp(a0x1)` → `     5`; `disp 0x1F` (command syntax) → `0x1F`; `y =
   0x2A` displays `y =`, a blank line, `    42` and a blank line. Cases:
   `hex_binary_in_expressions.m`.
4. Malformed literals, each through `eval` in `try` and `catch`,
   `disp(e.message)`: `x = 0x;` → `invalid number '0x'`; `x = 0xu8;` →
   `invalid number '0xu8'`; `x = 0b102;` → `invalid number '0b102'`;
   `x = 0x1Fz;` → `invalid number '0x1Fz'`; `x = 0x1Fu9;` → `invalid number
   '0x1Fu9'`; `x = 0x1Fi;` → `invalid number '0x1Fi'`; `x = 0x100u8;` →
   `invalid number '0x100u8'`; `x = 0x100s8;` → `invalid number
   '0x100s8'`; `x = 0x10000000000000000;` → `invalid number
   '0x10000000000000000'`. `err_*` cases in a script, exit 1, with no
   line of the script run: `disp(1)` then `x = 0b2` → `Error: Line 3:
   invalid number '0b2'` and nothing printed; and `x = 0x100u8`. Cases:
   `hex_binary_malformed.m`, `err_hex_binary_digit.m`,
   `err_hex_binary_too_large.m`.
5. Unit tests: token streams for each prefix and suffix, the two's
   complement of each signed suffix at its bounds, a value at 2^64 - 1 and
   at 2^64, leading zeros, and each malformed form. Tests:
   `hex_and_binary_literals_are_numbers`,
   `a_signed_suffix_reads_twos_complement`,
   `a_literal_fits_its_type_by_value`,
   `a_malformed_literal_is_an_invalid_number`.
6. Every existing case passes unchanged. Cases: the whole golden suite.

## Status

Planned
