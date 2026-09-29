# 11 — Strings and io

## Goal

`strcat strsplit strjoin strrep strtrim upper lower strcmp* strfind strtok num2str(matrices) int2str str2double str2num mat2str isspace isletter blanks regexp regexprep(hand-rolled) input fopen fclose fgetl fgets fprintf(fid) fread fwrite feof fileread readmatrix writematrix csvread csvwrite save load(MAT v5 uncompressed + -ascii) delete`

After this module a user can do things that were not possible before; every
bullet in Scope must be demonstrable by at least one acceptance test below.

## Scope

- String functions: `strcat`, `strsplit`, `strjoin`, `strrep`, `strtrim`,
  `upper`, `lower`, the `strcmp` family, `strfind`, `strtok`, `int2str`,
  `str2double`, `str2num`, `mat2str`, `isspace`, `isletter`, `blanks`
- `num2str` of a non-scalar (QA D13): one char row per matrix row with
  MATLAB's column widths, with and without a precision or a format.
  `num2str([1 2; 3 4])` is the 2x4 `'1  2'` / `'3  4'`, where today it is the
  1x10 `'1  3  2  4'`. The scalar forms `num2str(x, n)` and
  `num2str(x, formatSpec)` landed in cycle 01c
- A hand-rolled `regexp` and `regexprep`
- File input and output: `input`, `fopen`, `fclose`, `fgetl`, `fgets`,
  `fprintf(fid, ...)`, `fread`, `fwrite`, `feof`, `fileread`,
  `readmatrix`, `writematrix`, `csvread`, `csvwrite`, `delete`
- `fprintf` to the standard streams and its byte count (QA D25):
  `fprintf(1, ...)` writes to stdout, `fprintf(2, ...)` to stderr through the
  sink cycle 04's `warning` introduces, and `n = fprintf(...)` returns the
  number of bytes written. Today the first two are "The first argument must be
  a format string." and the third is "Too many output arguments."
- `save` and `load` for uncompressed MAT version 5, plus `-ascii`
- `printf` conversions and flags (QA D16), which cycle 01d's Out of scope
  moved here rather than leave half a `printf` rewrite in a bug-fix cycle:
  `%E` and `%G` printing a lower-case `e`; `%s` of a non-integer using `%g`
  where the MATLAB page's own example gives `%e`; the ignored `#` flag; the
  `0` flag padding a non-finite value; the unprocessed escapes `\xN`, `\N`
  (octal), `\a`, `\b`, `\f` and `\v`; `%x`, `%X`, `%o` and a `*` width or
  precision; and MATLAB printing the text up to an invalid conversion rather
  than erroring. See "Known bugs" in `docs/ARCHITECTURE.md` for each
- **`input` refuses where there is no terminal.** Under `--protocol` stdin
  is the protocol channel, so an `input` would swallow the next request;
  under `--ui` and `--http-stdio` there is no terminal at all. In all three
  `input` is a clean error with SplatCrab's own text, and the session goes
  on
- **The regular-expression engine runs in time linear in its input**
  (invariant 6): a Pike VM or Thompson NFA, never a backtracking matcher,
  so `regexp(repmat('a', 1, 1e5), '(a*)*b')` returns at once rather than
  hanging. Backreferences cannot be matched in linear time and are refused
  with a clean error. The supported syntax is recorded in the Design notes
- **`load` treats a MAT file as untrusted input**: every length and
  dimension it reads goes through `check_shape` before anything is
  allocated, and a truncated, corrupt or oversized file is a clean error,
  never a panic or an unbounded allocation
- **Every path resolves against `Interp::cwd`** (cycle 05's field, cycle
  13's rule): `fopen`, `fileread`, `readmatrix`, `writematrix`,
  `csvread`, `csvwrite`, `save`, `load` and `delete`. `delete` refuses a
  wildcard, which is safer than expanding one and is recorded as a
  deviation. A golden case that writes a file names it uniquely in its own
  directory and deletes it before it ends
- **`fprintf(2, ...)` goes through `Interp.err`**, so under `--protocol`
  and `--ui` it lands in an `eval`'s `out`, in order, as a warning does,
  and nothing reaches stderr there
- **A char range and `diag` of a char keep the char class**, the Known
  deviations row scheduled here: `'a':'e'` is `'abcde'` and `diag('ab')`
  is a 2x2 char
- **An unexpected character is named, not echoed**: the lexer's
  `unexpected character` message writes a control character as its code
  point, `U+0000`, instead of the raw byte, the Known bugs row that a
  UTF-16 file exposed. A printable character is still quoted as itself
- **Complex values**, which cycle 10 made: `num2str`, `mat2str`,
  `str2double`, `str2num` and the string functions take no complex
  argument; cycle 10's gate refuses one with its message. Formatting
  complex numbers as text is out of scope

Planning added the bullets above: two process modes cycle 11's `input`
and `fprintf(2, ...)` must respect, invariant 6 for the regex engine and the
MAT parser, the path rule, a Known deviations row and a Known bugs row
scheduled here, and the complex rule.

Two bullets that stood here, `%d` saturating at 64 bits and precision being
ignored for string conversions, were fixed by cycle 01d and removed from this
Scope in the same commit. That cycle also bounded `printf`'s width and
precision at `core::MAX_FIELD`; the rewrite here needs a bound of its own,
because the panics it prevents are in Rust's formatter and do not go away.

## Out of scope

- A char element as a UTF-16 code unit (QA D37). Cycle 02 chose that storage
  when it added the char class and took the row, `length('😀')` being `2`, with
  it; it and its acceptance test left this spec in 02's commit.

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.
- A full regular-expression engine if the hand-rolled one proves too large;
  in that case cut regexp/regexprep and record the cut here.
- Compressed MAT files if the hand-written inflate proves too large.

## Design notes

### Files and types

- `src/builtins/printf.rs` (new): the `printf` formatter, moved out of
  `core.rs` and finished for QA D16. `format_printf` and `MAX_FIELD` are
  re-exported from `core.rs`, so every caller is unchanged. `escapes` is the
  escape processing alone, which `strsplit` and `strjoin` use.
- `src/builtins/strings.rs` (new): the string functions, `num2str` (moved
  from `core.rs`), `int2str`, `mat2str`, `str2double`, `str2num`, and
  `regexp` and `regexprep`.
- `src/builtins/regex.rs` (new): the regular-expression engine, `Regex`.
- `src/builtins/io.rs` (new): `input`, the file-identifier table
  `FileTable` and `OpenFile`, the file functions, `fprintf` (moved from
  `core.rs`), `delete`, `save` and `load`.
- `src/builtins/mat.rs` (new): MAT-file version 5, `read` and `write`, pure
  functions over bytes.
- `src/interp.rs`: `Interp.input` (`InputSource::Stdin`, `Refused` or a
  `Reader` for tests) and `Interp.open_files`; `read_input_line`,
  `input_refused`, `eval_text` (what `input` evaluates), `str2num_value`,
  `resolve_path`, `files_changed`, `emit_bytes` and `emit_err_bytes`. A range
  whose two ends are chars is a char (`eval_range_end`).
- `src/parser.rs`: `Parser::parse_whole_expr`, one expression and nothing
  after it, for `input` and `str2num`.
- `src/protocol.rs`: `eval` puts `InputSource::Refused` in place for the
  length of every call, which covers `--protocol`, `--ui` and
  `--http-stdio`, since all three answer through it.
- `src/builtins/linalg.rs`: `diag` of a char keeps the char class, its
  zeros the character with code 0. A logical is still a double.
- `src/error.rs`: every new message, and `unexpected_char`, which names a
  control or invisible character (`char::is_control`, whitespace other than a
  space, and the soft hyphen, zero-width, bidirectional and word-joiner
  format characters) as `U+XXXX`.

### Invariants

Column-major storage throughout: `num2str`, `strtrim` and the other char
matrices are built by `char_rows`, which writes column-major; `fread` of
`[m n]` fills columns; `mat.rs` reads and writes column-major, as MAT-files
store. No index conversion was added outside `eval_index_args`: every
position these builtins return (`strfind`, `regexp`'s `start` and `end`) is
a zero-based code-unit offset plus one, computed where the value is built.
The `end` stack and name resolution are untouched. All output goes through
`Interp.out` and `Interp.err`: `fprintf(1, ...)` and `fwrite(1, ...)` through
`emit_bytes`, `fprintf(2, ...)` and `fwrite(2, ...)` through
`emit_err_bytes`, which flushes `out` first. Every message text is in
`error.rs`, and the scan that enforces it now covers the five new files.

Invariant 6, for input that is not the program's own:

- **The regular-expression engine is linear in its subject.** A Pike VM:
  one thread per program counter per position, in priority order, never a
  backtrack, so a search is `O(n * m)` for a subject of `n` code units and a
  program of `m` instructions. All the matches are found in one pass: the
  next search starts inside the pass as soon as the current one has a
  match, and a thread of a later search is dropped where an earlier search
  holds its program counter, which is what keeps `x*y|x` over a long run of
  `x`s linear rather than one rescan per match. The pattern is bounded
  too: nesting at `regex::MAX_NESTING` (250), since the parser and the
  compiler recurse once per level; a `{n,m}` count at `MAX_REPEAT` (1000);
  the compiled program at `MAX_PROGRAM` (20,000 instructions, each range
  of each distinct class counted as one), since a counted repetition
  copies its operand; the capture groups at
  `MAX_GROUPS` (100), since every thread carries two positions per group;
  and the matches handed back at `MAX_MATCH_ENTRIES` group positions in
  all, past which the search is `check_shape`'s error. Unit test:
  `a_pathological_pattern_on_a_long_subject_is_fast`.
- **A character class is built once.** When it is parsed it becomes
  sorted, merged, disjoint ranges of code units with its negation
  applied, tested by binary search; `\d`, `\w` and `\s` and their
  negations add their ranges once however often a class names them. Every
  copy a counted repetition makes shares the class by index, and so does
  a class written again with the same text. Cycle 11's review found each
  copy cloning the class and each code unit scanning its items: the class
  of ten thousand `b`s in `(?:[bb...b]{1000}){19}` took 2 GB and 12
  seconds to compile, where it is now one range and 19,000 instructions.
  A class of more ranges than `MAX_PROGRAM` leaves room for is `The
  regular expression is too large.` Under `'ignorecase'` a negated class
  matches a unit only when neither it nor its other case is one the class
  leaves out, so `[^a]` rejects `A`. Unit tests:
  `a_pathological_class_is_compiled_once_and_matched_fast` and
  `classes_are_normalised_ranges_with_negation_applied`.
- **The MAT reader treats the file as untrusted.** Every tag's length is
  checked against the bytes that are there before anything is sliced;
  every array's dimensions go through `check_shape` before anything is
  allocated, so `err_load_mat_huge_dims` is `check_shape`'s own message; a
  data element's length must be the array's count times its type's size
  before a value is converted; a cell never reserves more elements than
  the bytes left could hold, nor a struct more fields; a struct array with
  no fields, which no bytes of the file pay for, has at most
  `mat::MAX_FIELDLESS` (2^20) elements (see the deviations); and cells and
  structs nest at most `mat::MAX_DEPTH` (200) deep, in reading and in
  writing. The writer refuses a length a tag's 32 bits cannot hold, a
  variable past 4 GiB, rather than write it wrapped. Unit tests:
  `truncated_and_hostile_files_are_clean_errors`, which cuts a good file
  at every byte, `a_struct_array_with_no_fields_is_bounded_both_ways` and
  `a_length_past_four_gibibytes_is_refused_not_wrapped`.
- **No result is larger than its inputs allow without being judged.**
  `strrep`, `regexprep`, `strjoin` and `strcat` compute the length of what
  they would build first. `regexprep` computes it a token at a time, the
  matches times the tokens the replacement names rather than times its
  parts, and builds each replacement from the parts that write something
  there, so 20,000 `$0`s over 100,000 matches is refused at once with the
  whole size, where summing every part took minutes; a `$<` reads its
  name only as far as a name can go (unit tests
  `a_replacement_of_many_parts_is_sized_and_built_fast` and
  `many_named_token_starts_parse_in_linear_time`). `num2str`'s layout,
  `blanks`, `fread`'s `[m n]`,
  and the matrices `readmatrix`, `csvread` and `load -ascii` make from rows
  of text go through `check_shape`; `mat2str` and `printf` judge their text
  as it grows, since a bounded field cycled over many arguments can still
  ask for gigabytes. `fread` reads at most one element past the largest
  array, and `fileread` and `fgetl` judge the decoded text.
- **`printf` keeps cycle 01d's bound**: a width or precision past
  `MAX_FIELD` (8192), written or taken by `*` from an argument, is the same
  clean error, judged before Rust's formatter sees it.
- `strfind` and `strrep` search by Knuth-Morris-Pratt, so they are linear
  too, though the spec asks it only of the regular expressions.

### The regular-expression syntax supported

- Literals, and `\` before any character that is not an escape below,
  which is that character: `\.`, `\(`, `\\`.
- `.`, any code unit, the newline included (MATLAB's default `dotall`).
- Classes `[abc]`, `[a-z]`, `[^...]`, a `]` first in the class or a `-`
  first or last taken literally, and `\d \D \w \W \s \S` and the escapes
  below inside a class.
- `\d` (`[0-9]`), `\w` (a letter or digit of any script, or `_`), `\s`
  (`[ \f\n\r\t\v]`), and their negations `\D \W \S`.
- The escapes `\n \t \r \f \v \a \e \0`, `\b` as the backspace (MATLAB's
  meaning, not a word boundary), `\xN` and `\x{N}` in hexadecimal and `\oN`
  and `\o{N}` in octal, up to U+FFFF.
- Anchors `^` and `$`, the start and the end of the text (MATLAB's default
  `stringanchors`), and `\<` and `\>`, the start and the end of a word.
- Alternation `|`; groups `(...)`, non-capturing `(?:...)`, named
  `(?<name>...)`; comments `(?#...)`.
- Quantifiers `*`, `+`, `?`, `{n}`, `{n,}` and `{n,m}`, each lazy with a
  `?` after it. Stacked quantifiers, `a**`, are taken and nest. A `{` that
  does not start a count is a literal brace, as in Perl.
- Refused, each a clean error: backreferences `\1` to `\9` and `\k<name>`
  (`Backreferences are not supported in regular expressions.`), lookahead
  and lookbehind, atomic groups `(?>...)`, possessive quantifiers `a*+`,
  conditionals `(?(1)...)`, inline flags `(?i)` and every other `(?...)`.
- Options of `regexp`: `'match'`, `'tokens'`, `'names'`, `'start'`,
  `'end'`, `'split'`, `'tokenExtents'` (outputs, in the order given, and
  MATLAB's order start, end, tokenExtents, match, tokens, names, split when
  none is), `'once'`, `'ignorecase'`, `'matchcase'`, `'emptymatch'` and
  `'noemptymatch'`. `regexprep` takes the same options less the outputs;
  its replacement takes `$0`, `$N`, `$<name>`, `\$` and the escapes.
- Case folding for `'ignorecase'` is per code unit, ASCII and every
  single-character mapping in the BMP.

### The MAT-file types supported

Read: `mxDOUBLE_CLASS` and, as doubles, since SplatCrab has no other
numeric class, `single` and the eight integer classes; complex values;
logicals (the logical flag on any numeric class); chars stored as
`miUINT16`, `miUINT8`, `miINT8`, `miUTF8` or `miUTF16`; cells; structs and
struct arrays; numeric data stored under any of the ten numeric data types
MATLAB uses to save space; either byte order; small data elements; an
empty array written as an element with no bytes. Refused, each a clean
error naming the file: compressed data (`miCOMPRESSED`, MATLAB's default
since v7: save with `-v6` there), sparse arrays, objects and function
handles, and arrays of more than two dimensions. Written: doubles (complex
included) as `miDOUBLE`, logicals as `miUINT8` with the logical flag, chars
as `miUINT16`, cells and structs, little-endian, with a header naming
SplatCrab. A function handle or an `MException` cannot be saved.

`-ascii` writes each value `%.7e` right-aligned in sixteen columns (so
three spaces before a positive value), `%.16e` in twenty-five with
`-double`, and tab-separated with `-tabs`; `load -ascii` reads numbers
separated by whitespace or commas, `%` starting a comment, every line the
same length, and names the variable after the file (non-name characters
become `_`, and an `X` goes first when the name would not start with a
letter).

### Choices where the spec is silent

- **Cells of text** are taken by `strcat`, `strjoin` (which needs one),
  `strrep`, `strtrim`, `upper`, `lower`, the `strcmp` family, `strfind`,
  `str2double` and `regexprep`, each element by element, and by `regexp`,
  which answers a cell of per-element outputs. `strsplit`, `strtok`,
  `blanks`, `mat2str`, `int2str`, `num2str` and `str2num` take one array.
- **`num2str` of a matrix**: an integer matrix gives each column the width
  of its widest magnitude plus two; any other gives each element `%g` at
  `max(floor(log10(max|x|)) + 5, 5)` significant digits in a column that
  many plus seven wide, one more when an element is negative; the columns
  of spaces every row shares at the start are removed. The two layouts the
  acceptance tests pin are exact; the spacing of a negative integer and of
  a non-integer matrix is not verified against MATLAB and no case asserts
  it. With a precision, `%.ng` in columns `n + 7` wide (plus one for a
  negative); with a format, each row is `sprintf(format, row)`.
- **`strcat`** trims ASCII whitespace (space, `\t \n \v \f \r`) from the
  end of each char argument, a column only when it is whitespace in every
  row of a char matrix; with a cell among the arguments nothing is trimmed.
  A numeric argument is read as character codes.
- **`strsplit`** collapses delimiters by default, as MATLAB does, and takes
  only `'CollapseDelimiters'` among MATLAB's options; with several
  delimiters the longest that matches wins.
- **`strncmp(a, b, n)`** compares the first `min(n, length)` units of each,
  so `strncmp('abc', 'abc', 10)` is true and `strncmp('abc', 'abcd', 4)`
  false.
- **`strtrim`** removes whitespace (Unicode's, per code unit) and NUL.
  **`isspace`** is Unicode whitespace per code unit, **`isletter`**
  `char::is_alphabetic`.
- **`strfind`** of nothing found is `1x0`; **`strtok`**'s default
  delimiters are whitespace and NUL.
- **`mat2str`** writes `%.15g` (or `%.ng`), `true`/`false` for a logical,
  quoted rows for a char with quotes doubled; an empty is `zeros(r,c)`,
  `false(r,c)`, `''` for the 0x0 char and `char(zeros(r,c))` otherwise.
- **`str2double`** reads an optional sign, digits with an optional point,
  commas as thousands separators in the integer part, an exponent with `e`,
  `E`, `d` or `D`, and `Inf`, `Infinity` and `NaN` in any case; anything
  else, a complex number's text included, is `NaN`. A cell gives a matrix
  of its shape; a non-text value is `NaN`.
- **`str2num`** reads numbers, strings, the constants `pi`, `Inf`, `NaN`,
  `eps`, `true`, `false`, `i` and `j` (and `inf` and `nan`), brackets,
  ranges and operators, and nothing that calls a function or reads a
  variable, so text from a file cannot run code; `[x, ok] = str2num(...)`
  is `[]` and false otherwise. It evaluates in a workspace of its own that
  holds only the constants the text names, each the builtin's value,
  taken from the builtin directly, so neither a variable nor a file on
  the path shadows one: cycle 11's review found `str2num('pi')` running a
  `pi.m` (unit test
  `str2num_reads_its_constants_from_the_builtins_never_the_path`). MATLAB
  hands the text to `eval`: a deviation.
- **`int2str`** rounds half away from zero and lays a matrix out as
  `num2str` does.
- **`input`** writes its prompt, flushes, and reads one line through the
  standard-input buffer the REPL shares; an empty line is `[]`; text that
  is not an expression is the parse error, where MATLAB asks again; the end
  of input is `'input' reached the end of standard input.`; a second
  argument other than `'s'` is refused. Where there is no terminal the
  refusal comes before the prompt is written, so the `out` of the answer is
  empty.
- **File identifiers**: the lowest free from 3 up. `0`, `1` and `2` are
  valid for `fprintf` and `fwrite` (`1` and `2`), and `Invalid file
  identifier.` for every function that reads or closes; `fclose` returns
  `0`, and `fclose('all')` closes every file. `fopen` takes the
  permissions `r w a r+ w+ a+` with at most one `t` or `b`; it takes no
  machine format or encoding, and text is UTF-8 both ways. `fopen` of a
  file it cannot open is `-1` and a reason, never an error; of a folder,
  `It is a directory`. With `t`, `fgetl` also drops a carriage return
  before the newline; nothing is translated on writing.
- **`feof`** is set by a read that reaches the end of the file, including
  one that ends exactly there, so `while ~feof(fid)` stops after the last
  line whether or not a newline ends it; it is `0` before any read. It
  returns a double.
- **`fread`** takes `Inf` (the default), `n` or `[m n]` (`n` may be `Inf`),
  pads the last column with zeros, and returns `[A, count]`; precisions
  `uint8 uchar char int8 schar uint16 int16 uint32 int32 uint64 int64 single
  float32 double float64` and C's spellings, little-endian, with `*` or
  `=>char` giving a char. **`fwrite`** rounds and saturates to the integer
  types, `NaN` as `0`, and returns the element count.
- **`writematrix`** writes up to 15 significant digits (`%.15g`), comma
  separated unless `'Delimiter'` names another, to `matrix.txt` when no file
  is named; **`csvwrite`** writes five, with its row and column offsets as
  empty lines and empty fields. Both refuse a char, a cell and a struct.
- **`readmatrix`** takes the first of a comma, a tab and a semicolon the
  file holds as the delimiter, else whitespace; skips blank lines and the
  leading lines with no number in them; an empty or non-numeric field and a
  short row's missing fields are `NaN`. **`csvread`** gives `0` for an
  empty field and a missing one and refuses any other non-number, naming
  its line.
- **`delete`** takes one or more files by name; a name with `*` is
  `Wildcards are not supported by 'delete'.` before anything is deleted; a
  file that is not there is the warning `File 'x' not found.`, and the
  others are still deleted.
- **`save`** saves every variable, sorted by name, when none is named, to
  `matlab.mat` when no file is named, and adds `.mat` to a MAT-file name
  with no extension; it takes `-mat`, `-v6` and `-v7` (each an uncompressed
  version 5 file), `-append`, and `-ascii` with `-double` and `-tabs`.
  With nothing to write, an empty workspace and no `-append` file to keep,
  it is `There are no variables to save.`: the file would be a header
  alone, which item 16 makes `load` refuse, and `save` never writes a file
  its own `load` cannot read. For the same reason it refuses a struct
  array with no fields past `mat::MAX_FIELDLESS` elements, and a variable
  larger than the 4 GiB whose length a tag can hold, before it writes
  anything. **`load`** tries `name.mat` when `name` is
  not there, reads a MAT-file for `-mat`, a `.mat` name or a file that
  starts with the version 5 header, and text otherwise; a variable asked
  for that the file does not hold is a warning. A name the file holds
  twice is one variable with the last value, in the place of the first, so
  `S = load(...)` never has a field twice. A length that runs past the end
  of the file is `the file is truncated`; one that runs past the end of the
  element holding it, in a file that is all there, is `its data is
  corrupt`.
- **A folder named as a file** is `It is a directory` for every function
  that reads, writes or deletes one, checked before the operating system
  is asked, since Windows would answer `Permission denied` where Linux
  answers the truth. `delete` of a folder deletes nothing.
- **Text is judged as it is read**: `fileread`, `fgetl`, `fgets` and
  `input` stop at `3 * MAX_ELEMS` bytes (`io::MAX_TEXT_BYTES`), since three
  bytes of UTF-8 hold one code unit at most and text past that is too long
  for a char row whatever it holds; the error is `check_shape`'s. The rows
  of numbers `readmatrix`, `csvread` and `load -ascii` gather go through
  `check_shape` as each is added, so a huge text file is refused before its
  numbers take more memory than the largest array could.
- **Open files** have no limit of SplatCrab's own: identifiers go on from 3
  until the operating system refuses a handle, when `fopen` is `-1` and a
  reason. 10,000 files open at once was tried.
- **`fprintf`** reads a numeric first argument followed by more as a file
  identifier, and refuses an identifier that names nothing before it
  formats; `n = fprintf(...)` counts UTF-8 bytes.
- **The lexer's message** names a character that would not show as
  `U+XXXX`, four hex digits at least; every other character is quoted as
  itself.

### Texts settled in testing

The acceptance tests left four texts to the implementation; each is now
pinned in full in its case, as are the two warnings and the texts cycle
11's review added:

- Item 14, `input` where there is no terminal: `input is not available in
  this session: there is no terminal to read from.`
- Item 15, a backreference: `Backreferences are not supported in regular
  expressions.`
- Item 16, a MAT-file that cannot be loaded: `Unable to read MAT-file
  '<name>': <reason>.`, the reason one of `the file is truncated`, `the
  file is truncated after its header`, `it is not a MAT-file of version
  5`, `it holds compressed data, which SplatCrab does not read (save with
  -v6)`, `its data is corrupt`, `an array in it has more than two
  dimensions`, `its cells and structs are nested too deeply`, `it holds a
  sparse array, which SplatCrab does not read`, `it holds an object or a
  function handle, which SplatCrab does not read` and, since cycle 11's
  review, `it holds a struct array with no fields and more than 1048576
  elements`; dimensions past the limit are `check_shape`'s `Requested
  1000000x1000000 array exceeds the maximum array size.` A file that ends
  right after its header is an error, as item 16 reads literally.
- Item 18, a wildcard: `Wildcards are not supported by 'delete'.`
- Item 17: a control character is named unquoted, `unexpected character
  U+0000`; a printable one is quoted, `unexpected character '$'`.
- Item 18's two warnings, pinned by `delete_and_load_missing_warn`:
  `Warning: File 'x' not found.` from `delete` of a file that is not
  there, and `Warning: Variable 'q' not found.` from `load` of a variable
  the file does not hold.
- Item 9, from cycle 11's review: `save` of a struct array with no
  fields past the bound is `Unable to save variable 's': a struct array
  with no fields and more than 1048576 elements cannot be saved.`, and of
  a variable past 4 GiB `Unable to save variable 'x': it is larger than
  the 4 GiB a MAT-file of version 5 can hold in one element.`, the second
  pinned by a unit test alone, since no script can build the value.

Every other message the cycle added is SplatCrab's own and pinned by an
`err_*` case: `err_regexp_refused_constructs`,
`err_regexp_syntax_and_bounds`, `err_regexp_class_too_large`,
`err_input_end_of_stdin`,
`err_input_second_argument`, `err_fopen_permission`,
`err_file_open_modes`, `err_whole_file_errors`, `err_save_refusals`,
`err_save_fieldless_struct_bound`, `err_load_mat_faults`,
`err_load_mat_fieldless_struct_bound`, `err_load_ascii_faults` and
`err_string_function_arguments`, and the two warnings by
`delete_and_load_missing_warn`.

### Deviations accepted

- `delete` refuses a wildcard rather than expand it.
- Backreferences, lookaround, atomic groups, possessive quantifiers,
  conditionals and inline flags are refused; `regexpi` does not exist.
- `str2num` reads literals and operators, never `eval`.
- `input` of text that is not an expression is an error, where MATLAB asks
  again.
- A compressed MAT-file is refused; the integer and `single` classes load
  as doubles.
- A struct array with no fields has at most `mat::MAX_FIELDLESS`
  elements, 2^20 (1,048,576), in a MAT-file `load` reads or `save` writes;
  MATLAB reads and writes any size. Every other array's elements are paid
  for by the file's bytes, which the reader bounds them by, but one with no
  fields holds no data: MATLAB writes a 1-by-N one in a few bytes whatever
  N is, so no byte-based bound fits it. Each element still costs an empty
  field list, 24 bytes, so a 200-byte file claiming 16384-by-16384 took
  6 GB and 51 seconds to load, and aborts in the allocator on a smaller
  machine. 2^20, a 256th of the element limit, holds such an array to
  24 MB and milliseconds, and no honest file holds a million elements of
  a placeholder that has no fields; `save` keeps to it so that it never
  writes a file its own `load` refuses.
- `save` in an empty workspace is an error rather than a file of a header
  alone.
- Several message texts are SplatCrab's own: the regular-expression
  refusals and syntax errors, `input`'s, the file errors
  (`Unable to read file 'x': No such file or directory.` and its kind, with
  the reason in words that are the same on every platform), the MAT-file
  errors and `delete`'s.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/11-strings-and-io/`. Expected output is written by hand from MATLAB
semantics wherever feasible; the `.err` substring is always chosen by hand.
Every new error message needs an `err_*` case.

1. `disp(strcat('a', 'b', 'c')); disp(strcat('a ', 'b')); disp(['a ' 'b'])` → `abc\nab\na b` Cases: strcat_trailing_space.
2. `c = strsplit('a,b,c', ','); disp(numel(c)); disp(c{3}); disp(strjoin({'a', 'b'}, '-'))` → `     3\nc\na-b` Cases: strsplit_strjoin, err_string_function_arguments.
3. `disp(strrep('hello world', 'o', '0')); disp(upper('abc')); disp(strtrim('  x  ')); disp(fliplr('abc')); disp(strfind('abcabc', 'bc'))` → `hell0 w0rld\nABC\nx\ncba\n     2     5` Cases: strrep_upper_strtrim_strfind, err_upper_complex_input.
4. `disp(strcmp('a', 'a')); disp(strcmp('a', 'b')); disp(strcmpi('A', 'a')); disp(strcmp({'a', 'b'}, 'a')); disp('abc' == 'abd')` → `   1\n   0\n   1\n   1   0\n   1   1   0` (`strcmp` returns a logical, four wide since cycle 02) Cases: strcmp_family_logical, string_lower_strncmp_isspace_blanks.
5. `disp(str2double('3.14')); disp(str2double('abc')); disp(str2num('[1 2 3]')); disp(num2str(pi)); disp(num2str([1 2 3])); disp(int2str(2.7))` → `    3.1400\n   NaN\n     1     2     3\n3.1416\n1  2  3\n3` Cases: str2double_str2num_num2str_int2str, str2num_constants_ignore_the_path, err_num2str_complex_input, err_str2double_complex_input.
6. `disp(mat2str([1 2; 3 4])); disp(mat2str([1.5 2])); disp(sprintf('%d', [1 2 3])); disp(sprintf('%5.2f|%-4d|%s', pi, 7, 'ab')); fprintf('100%%\n')` → `[1 2;3 4]\n[1.5 2]\n123\n 3.14|7   |ab\n100%` Cases: mat2str_sprintf_fprintf_percent, printf_upper_exponent_and_s_of_fraction, printf_hash_flag_and_zero_pad_nonfinite, printf_escapes_hex_octal_control, printf_hex_octal_and_star, printf_invalid_conversion_truncates, err_printf_star_width_bounded, err_printf_star_precision_bounded, err_mat2str_complex_input.
7. Case with `.stdin` = `42\nBob\n`: `x = input('n: '); s = input('name: ', 's'); fprintf('%d %s\n', x * 2, s)` → `n: name: 84 Bob` Cases: input_number_and_string, err_input_end_of_stdin, err_input_second_argument.
8. `fid = fopen('out.txt', 'w'); fprintf(fid, 'line1\nline2\n'); fclose(fid); fid = fopen('out.txt'); l = fgetl(fid); disp(l); l = fgetl(fid); l = fgetl(fid); disp(l); fclose(fid); delete('out.txt')` → `line1\n    -1` Cases: fopen_fprintf_fgetl, fgets_feof, fwrite_fread_round_trip, err_fopen_permission, err_file_open_modes.
9. `x = [1 2; 3 4]; s = 'hi'; c = {1, 'a'}; save('t.mat', 'x', 's', 'c'); clear; load('t.mat'); disp(x); disp(s); disp(class(c)); delete('t.mat')` → `     1     2\n     3     4\nhi\ncell` Cases: save_load_mat_round_trip, save_load_ascii, err_save_refusals, err_save_fieldless_struct_bound, err_load_ascii_faults.
10. `writematrix([1 2; 3 4], 'm.csv'); disp(readmatrix('m.csv')); disp(fileread('m.csv')); delete('m.csv')` → `     1     2\n     3     4\n1,2\n3,4` Cases: writematrix_readmatrix_fileread, csvwrite_csvread, err_whole_file_errors.
11. `disp(regexprep('abc123', '\d', '')); [tok, rest] = strtok('hello world'); disp(tok); disp(rest)` → `abc\nhello\n world` Cases: regexprep_strtok, regexp_match_tokens_and_indices, err_regexprep_long_replacement.
12. `s = num2str([1 2; 3 4]); disp(size(s)); disp(s); disp(num2str([1; 22]))` → `     2     4\n1  2\n3  4\n 1\n22` Cases: num2str_matrix_rows, num2str_matrix_precision_and_format.
13. `fprintf(1, 'hi\n'); n = fprintf('ab\n')` → `hi\nab\nn =\n\n     3\n`; `fprintf(2, 'to stderr\n')` writes `to stderr` to stderr and nothing to stdout; `fprintf(7, 'x')` with no such file open → err `Invalid file identifier.` Cases: fprintf_fid1_and_byte_count, fprintf_fid2_to_stderr, err_fprintf_invalid_fid.
14. Under `--protocol`, `{"id":1,"op":"eval","code":"x = input('n: ');"}` → an error answer with SplatCrab's text pinned in the case, exit 0, nothing on stderr; and `{"id":2,"op":"eval","code":"fprintf(2, 'e\\n'); disp(1)"}` → `"out":"e\n     1\n"` Cases: err_input_refused_under_protocol, err_input_refused_under_http_stdio.
15. `tic; r = regexp(repmat('a', 1, 100000), '(a*)*b', 'match'); disp(isempty(r)); disp(toc < 5)` → `   1\n   1`; `regexp('aa', '(a)\1')` → a clean error, exit 1, with SplatCrab's text pinned Cases: regexp_nested_star_linear_time, err_regexp_backreference, err_regexp_refused_constructs, err_regexp_syntax_and_bounds, err_regexp_class_too_large.
16. A MAT file truncated after its header, and one whose array header claims 1e12 elements, each loaded → a clean error, exit 1 (write the bytes with `fwrite` in the case itself); neither aborts Cases: err_load_mat_truncated, err_load_mat_huge_dims, err_load_mat_header_only, err_load_mat_element_size_overruns, err_load_mat_faults, err_load_mat_fieldless_struct_bound.
17. `disp('a':'e'); disp(class('a':'c')); d = diag('ab'); disp(class(d)); disp(size(d))` → `abcde\nchar\nchar\n     2     2` Cases: char_range_and_diag_keep_char, err_lexer_control_character_named, err_lexer_printable_character_quoted.
18. `fid = fopen('delete_wild_test.txt', 'w'); fclose(fid); delete('*.txt')` → a clean error, exit 1, with SplatCrab's text pinned; the case deletes its file by name first so it leaves nothing behind Cases: err_delete_wildcard, delete_and_load_missing_warn.

## Status

Done (2026-09-29)
