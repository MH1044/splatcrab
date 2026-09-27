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
- A char element is a UTF-16 code unit, as in MATLAB (QA D37):
  `length('😀')` is `2`
- `save` and `load` for uncompressed MAT version 5, plus `-ascii`
- Fix `%d` saturating at 64 bits: `fprintf('%d', 1e30)` prints the `i64`
  clamp `9223372036854775807` instead of the full value. See "Known bugs"
  in `docs/ARCHITECTURE.md`
- Fix precision being ignored for string conversions: `%5.2s` of
  `'abcdef'` must truncate to two characters before padding to five

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.
- A full regular-expression engine if the hand-rolled one proves too large;
  in that case cut regexp/regexprep and record the cut here.
- Compressed MAT files if the hand-written inflate proves too large.

## Design notes

Filled in as decisions are made. Start from the
"Key designs to preserve" section of docs/ARCHITECTURE.md and record here:
which files and types changed, which invariants were preserved (column-major
storage, 1-based to 0-based conversion at the index boundary, the `end` stack,
name resolution order, the output sink), and any deviation from MATLAB that
was accepted deliberately.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/11-strings-and-io/`. Expected output is written by hand from MATLAB
semantics wherever feasible; the `.err` substring is always chosen by hand.
Every new error message needs an `err_*` case.

1. `disp(strcat('a', 'b', 'c')); disp(strcat('a ', 'b')); disp(['a ' 'b'])` → `abc\nab\na b`
2. `c = strsplit('a,b,c', ','); disp(numel(c)); disp(c{3}); disp(strjoin({'a', 'b'}, '-'))` → `     3\nc\na-b`
3. `disp(strrep('hello world', 'o', '0')); disp(upper('abc')); disp(strtrim('  x  ')); disp(fliplr('abc')); disp(strfind('abcabc', 'bc'))` → `hell0 w0rld\nABC\nx\ncba\n     2     5`
4. `disp(strcmp('a', 'a')); disp(strcmp('a', 'b')); disp(strcmpi('A', 'a')); disp(strcmp({'a', 'b'}, 'a')); disp('abc' == 'abd')` → `     1\n     0\n     1\n   1   0\n   1   1   0`
5. `disp(str2double('3.14')); disp(str2double('abc')); disp(str2num('[1 2 3]')); disp(num2str(pi)); disp(num2str([1 2 3])); disp(int2str(2.7))` → `    3.1400\n   NaN\n     1     2     3\n3.1416\n1  2  3\n3`
6. `disp(mat2str([1 2; 3 4])); disp(mat2str([1.5 2])); disp(sprintf('%d', [1 2 3])); disp(sprintf('%5.2f|%-4d|%s', pi, 7, 'ab')); fprintf('100%%\n')` → `[1 2;3 4]\n[1.5 2]\n123\n 3.14|7   |ab\n100%`
7. Case with `.stdin` = `42\nBob\n`: `x = input('n: '); s = input('name: ', 's'); fprintf('%d %s\n', x * 2, s)` → `n: name: 84 Bob`
8. `fid = fopen('out.txt', 'w'); fprintf(fid, 'line1\nline2\n'); fclose(fid); fid = fopen('out.txt'); l = fgetl(fid); disp(l); l = fgetl(fid); l = fgetl(fid); disp(l); fclose(fid); delete('out.txt')` → `line1\n    -1`
9. `x = [1 2; 3 4]; s = 'hi'; c = {1, 'a'}; save('t.mat', 'x', 's', 'c'); clear; load('t.mat'); disp(x); disp(s); disp(class(c)); delete('t.mat')` → `     1     2\n     3     4\nhi\ncell`
10. `writematrix([1 2; 3 4], 'm.csv'); disp(readmatrix('m.csv')); disp(fileread('m.csv')); delete('m.csv')` → `     1     2\n     3     4\n1,2\n3,4`
11. `disp(regexprep('abc123', '\d', '')); [tok, rest] = strtok('hello world'); disp(tok); disp(rest)` → `abc\nhello\n world`
12. `s = num2str([1 2; 3 4]); disp(size(s)); disp(s); disp(num2str([1; 22]))` → `     2     4\n1  2\n3  4\n 1\n22`
13. `fprintf(1, 'hi\n'); n = fprintf('ab\n')` → `hi\nab\nn =\n\n     3\n`; `fprintf(2, 'to stderr\n')` writes `to stderr` to stderr and nothing to stdout; `fprintf(7, 'x')` with no such file open → err `Invalid file identifier.`
14. `disp(length('😀'))` → `     2`

## Status

Planned
