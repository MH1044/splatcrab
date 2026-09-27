# 12 — Plotting

## Goal

`src/plot/{figure,svg,png}.rs`; `figure gcf close clf subplot plot scatter bar histogram xlabel ylabel title legend grid axis xlim ylim hold saveas print`; SVG writer first, PNG via store-only zlib; REPL opens the SVG with the OS viewer; goldens read the SVG back with `fileread`/`strfind`

After this module a user can do things that were not possible before; every
bullet in Scope must be demonstrable by at least one acceptance test below.

## Scope

- A plotting module in `src/plot/`, split into `figure`, `svg` and `png`
- The builtins `figure`, `gcf`, `close`, `clf`, `subplot`, `plot`,
  `scatter`, `bar`, `histogram`, `xlabel`, `ylabel`, `title`, `legend`,
  `grid`, `axis`, `xlim`, `ylim`, `hold`, `saveas` and `print`
- An SVG writer first; PNG through a store-only zlib encoder
- The command-line runner hands a saved SVG to the operating system viewer
- A figure's rendered SVG is retrievable in memory, not only through a file:
  `Interp` keeps the SVG of each open figure addressable by number, through
  `figure_svg(n)` and `figure_numbers()`, so a front end can render it inline
  rather than writing a temporary file and reading it back. The writer already
  builds the whole SVG as a `String` before `saveas` writes it, so this keeps
  what exists rather than producing anything new
- Golden cases read the saved SVG back with `fileread` and `strfind`, so
  no image comparison is needed

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.
- **Interaction with a figure**: no zoom, no pan, no data cursor, no click
  handling, in the terminal or in the interface. A figure is a rendered image.
  The command-line runner hands the file to the operating system viewer; the U
  series renders the same SVG inline in a figure pane. Neither is interactive.
  (This bullet used to forbid "an interactive figure window" and then define
  the output as files only, which read as a ban on showing a plot anywhere but
  an external viewer. The intent was always to rule out interaction, not
  display.)
- 3-D plots, surfaces and animation.

## Design notes

Filled in as decisions are made. Start from the
"Key designs to preserve" section of docs/ARCHITECTURE.md and record here:
which files and types changed, which invariants were preserved (column-major
storage, 1-based to 0-based conversion at the index boundary, the `end` stack,
name resolution order, the output sink), and any deviation from MATLAB that
was accepted deliberately.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/12-plotting/`. Expected output is written by hand from MATLAB
semantics wherever feasible; the `.err` substring is always chosen by hand.
Every new error message needs an `err_*` case.

1. `plot(1:3, [1 4 9]); saveas(gcf, 'p1.svg'); s = fileread('p1.svg'); disp(numel(strfind(s, '<polyline'))); disp(~isempty(strfind(s, '>9<')))` → `     1\n     1`
2. `hold on; plot(1:3, 1:3); plot(1:3, 2:4); hold off; saveas(gcf, 'p2.svg'); disp(numel(strfind(fileread('p2.svg'), '<polyline')))` → `     2`
3. `subplot(2, 1, 1); plot(1:2); subplot(2, 1, 2); plot(1:3); saveas(gcf, 'p3.svg'); disp(numel(strfind(fileread('p3.svg'), 'class="axes"')))` → `     2`
4. `plot(1:2); xlabel('t'); ylabel('y'); title('T'); legend('a'); saveas(gcf, 'p4.svg'); s = fileread('p4.svg'); fprintf('%d %d %d %d\n', ~isempty(strfind(s, '>t<')), ~isempty(strfind(s, '>y<')), ~isempty(strfind(s, '>T<')), ~isempty(strfind(s, '>a<')))` → `1 1 1 1`
5. `scatter([1 2 3], [3 1 2]); saveas(gcf, 'p5.svg'); disp(numel(strfind(fileread('p5.svg'), '<circle')))` → `     3`
6. `bar([1 2 3]); saveas(gcf, 'p6.svg'); disp(numel(strfind(fileread('p6.svg'), '<rect class="bar"')))` → `     3`
7. `histogram([1 1 2 3 3 3], 3); saveas(gcf, 'p7.svg'); disp(numel(strfind(fileread('p7.svg'), '<rect class="bar"')))` → `     3`
8. `plot(1:3, 1:3, 'r--'); saveas(gcf, 'p8.svg'); s = fileread('p8.svg'); fprintf('%d %d\n', ~isempty(strfind(s, 'stroke="#ff0000"')), ~isempty(strfind(s, 'stroke-dasharray')))` → `1 1`
9. `plot([1 2; 3 4; 5 6]); saveas(gcf, 'p9.svg'); disp(numel(strfind(fileread('p9.svg'), '<polyline')))` → `     2`
10. `plot(1:3); print('-dpng', 'p.png'); fid = fopen('p.png'); b = fread(fid, 8); fclose(fid); disp(b')` → `   137    80    78    71    13    10    26    10`
11. `figure; figure; disp(gcf)` → `     2`; `close all; figure; disp(gcf)` → `     1`
Each plotting case deletes the files it wrote.

## Status

Planned
