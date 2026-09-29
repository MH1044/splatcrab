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
- An SVG writer first; PNG through a store-only zlib encoder over a
  rasterizer of the figure's own: lines, markers, bars and axes are drawn
  into pixels, and text with a small built-in bitmap font (no font file,
  no crate). The image's pixel size, and any resolution `print` is given,
  go through `check_shape`, so a huge `-r` is a clean error
- **`gcf` returns the current figure's number as a double**, as MATLAB
  did before R2014b, since this interpreter has no graphics objects.
  Since R2014b MATLAB returns a Figure object, and `disp(gcf)` lists its
  properties; that is a recorded deviation
- **Bounded work** (invariant 6): time and memory grow linearly in the
  points plotted, and a plot the output cannot hold is refused cleanly
- A complex argument to a plotting builtin is refused by cycle 10's gate;
  MATLAB plots the real against the imaginary part, which is a recorded
  deviation
- `close all`, `hold on` and `grid on` work as command syntax (cycle 04),
  now that `close`, `hold` and `grid` exist
- Every path through `Interp::cwd`; every plotting case deletes the files
  it writes
- **The viewer opens only from an interactive REPL.** When stdin is a
  terminal (`std::io::IsTerminal`, in the standard library), a figure
  drawn at the prompt is written to a temporary SVG and handed to the
  operating system viewer, and any failure to open it is ignored. A
  script, a golden case, CI, `--protocol`, `--ui` and `--http-stdio`
  never open a viewer: otherwise every plotting case would spawn one.
  (The bullet that stood here said "the command-line runner", which
  would have included scripts; narrowed at planning.)
- A figure's rendered SVG is retrievable in memory, not only through a file:
  `Interp` keeps the SVG of each open figure addressable by number, through
  `figure_svg(n)` and `figure_numbers()`, so a front end can render it inline
  rather than writing a temporary file and reading it back. The writer already
  builds the whole SVG as a `String` before `saveas` writes it, so this keeps
  what exists rather than producing anything new. No golden case can reach
  the accessor, so a unit test pins it; a protocol operation for it is
  cycle U4's
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

### Files and types

- `src/plot/mod.rs`: the twenty builtins and their argument rules,
  registered from `builtins::registry` through `plot::register`. They only
  change figure state; nothing is drawn until a figure is saved, printed or
  read through the accessor.
- `src/plot/figure.rs`: `Figures` (on `Interp` as `figures`), `Figure`,
  `Axes`, `Place`, `Series` (`Line`, `Scatter`, `Bars`), `LineSpec` and
  `parse_line_spec`, the tick rule, and `scene`, the layout of a figure into
  a `Scene` of `Item`s in pixels of the 560x420 figure, y downwards.
- `src/plot/svg.rs`: `render(&Scene) -> String`.
- `src/plot/png.rs`: `pixel_size`, the `Canvas` rasterizer, the bitmap
  font, `crc32`, `adler32`, `zlib_store` and `encode`.
- `src/interp.rs`: the `figures` field and three public methods,
  `figure_numbers()`, `figure_svg(n)` and `take_changed_figures()`.
- `src/main.rs`: the REPL's viewer, `show_figures`.
- `src/error.rs`: sixteen new texts (below); `src/builtins/io.rs`:
  `write_file` is `pub(crate)`, so `saveas` and `print` write through it and
  so through `Interp::resolve_path` and `files_changed`.

SVG and PNG read the one `Scene`, so the two formats cannot disagree about
where anything is. The invariants: no matrix storage or indexing changed,
so column-major storage, the one-based boundary, the `end` stack and name
resolution are untouched; the builtins print nothing, so the output sink is
untouched; every message text is in `error.rs`, and the scan test covers
the four new files. `print!` appears in none of them.

### State

A figure number is a `u32` from 1 to 2,147,483,647; anything else, and a
number naming no open figure, is `Invalid figure handle.` `figure` makes the
lowest unused number current. The current figure is the last of a
focus order, so closing it makes the one current before it current again,
as MATLAB's does. `gcf`, and every builtin that draws or decorates, makes
figure 1 when none is open, and the current axes when the figure has none
(a whole-figure axes). With hold off, `plot`, `scatter`, `bar` and
`histogram` reset the axes to a new one at the same place, labels, legend,
grid, limits, `axis` modes and colour order included: MATLAB's `NextPlot`
`'replace'`. `hold` is the axes', not the figure's.

`subplot(m, n, p)` numbers cells along the rows from the top left; a
vector `p` is the smallest rectangle holding its cells; `subplot(mnp)` is
three digits. An axes at the same place is made current again; a new one
deletes every axes whose rectangle it overlaps (the whole-figure axes
included), so `subplot(1, 1, 1)` is the whole-figure axes.

### The SVG vocabulary

- `<?xml ...?>`, then `<svg xmlns=... width="560" height="420" viewBox=...>`
  and a white `<rect class="figure">`.
- Each axes is one `<g class="axes">`, the only element with that class. In
  it: a `<rect class="box">`, `<line class="grid">` at the ticks when the
  grid is on, a `<clipPath>` and a `<g class="data">` clipped to the plot
  box holding the series, a `<rect class="frame">`, `<line class="tick">`
  inward on all four sides, the tick labels, the axis labels, the title and
  a `<g class="legend">`.
- A run of a line (a stretch of finite points) is one
  `<polyline class="line" fill="none" stroke="#rrggbb" stroke-width="1"
  points=...>`, with `stroke-dasharray` `6,4` for `--`, `1.5,3` for `:` and
  `6,3,1.5,3` for `-.`. Nothing else is a `<polyline>`.
- A marker `o` or `.` and every scatter point is a `<circle>`; `s`, `d`
  and the triangles and stars are `<polygon class="marker">`; `+`, `x` and
  `*` are `<path class="marker">`. A legend's sample is a `<line>`, an
  `<ellipse>` for a circle and a `<rect class="sample">` for bars, so a
  legend never adds to the counts the cases make.
- A bar is `<rect class="bar" ... fill=... stroke="#000000">`, with
  `fill-opacity="0.6"` for a histogram's.
- A text is `<text class=... x y font-size text-anchor fill>` whose content
  is the text alone, `&`, `<`, `>` and `"` escaped and a control character
  replaced, so `xlabel('t')` is `>t<`. A y label is rotated with
  `transform="rotate(-90 x y)"`; a title is bold.
- Numbers have at most two decimals, trailing zeros dropped. A pixel
  coordinate is clamped to plus or minus 1,000,000, so a point far past the
  limits writes a short number and costs the rasterizer only its clipped
  part.

Colours: a series with none takes MATLAB's R2014b colour order in turn,
`#0072bd #d95319 #edb120 #7e2f8e #77ac30 #4dbeee #a2142f`, carried on under
hold; the letters `rgbcmykw` are the pure colours (`r` is `#ff0000`); the
axes and text are `#262626` and the grid `#dfdfdf`.

### Layout

The whole-figure axes' plot box is MATLAB's default position
`[0.13 0.11 0.775 0.815]`. A subplot's is the same margins inside its cell,
each at most a share of the cell (left 25%, right 10%, bottom 30%, top
20%), so the formula gives the default box for `subplot(1, 1, 1)`. `axis
square` shrinks the box to a square about its centre, and `axis equal`
widens the limits of one axis so a unit is as long on both. A legend sits
in the plot box's top right corner; a label with no series to name is not
drawn. Limits of a manual end with an infinite other end keep the automatic
one, as MATLAB's do.

### The tick rule

For each axis, `n` is the plot box's width over 50 pixels (its height over
30 for y), clamped to 2..=10. The step is the smallest of `1, 2, 5` times
a power of ten that splits the range into at most `n` intervals. Automatic
limits are the data's extent widened outwards to multiples of the step, so
both ends carry a tick; `axis tight` and manual limits keep their ends and
tick every multiple of the step inside them. With no data the limits are
`[0 1]`; a single value `v` gets `v - 1` to `v + 1` (wider for a huge
`v`). A tick label is built from the multiple's integer and the step's
decimal exponent, never from a float's digits, so `9` is `9`, `0.3` is
`0.3` and `2.5` is `2.5`; a label longer than ten characters is `%g`
instead (`1e+12`). When no step can be found (a range that overflows, or
multiples past `1e15`) the ticks are the two ends in `%g`. There are at
most eleven ticks an axis, whatever the limits.

### The font and the rasterizer's coverage

The font is 5x7 pixels in a 6x8 cell, one glyph for each printable ASCII
character, written as bit rows in `png.rs`, and a hollow box for every
other character. A text scales by the whole number nearest its size times
the resolution over 10, at least 1; bold is drawn twice a pixel apart; a
vertical text reads upwards.

The rasterizer draws everything the SVG holds: rectangles filled with
their opacity (blended over what is there) and outlined; lines and
polylines walked a pixel at a time along the longer side, with the dash
pattern carried on across a polyline's vertices; circles filled row by row
and outlined; marker polygons and strokes; texts; each clipped to its
group's box. There is no anti-aliasing and no curve but the circle. A
segment is clipped to its box (Liang and Barsky) before it is walked, and
the dash pattern is walked only over the clipped part, so a segment costs
at most the box's diagonal. A line up to 1.5 pixels wide is a pixel at each
step; a wider one is the union of the squares of its width centred on the
steps, drawn as one run across the line for each pixel along it, so it
costs the pixels it covers, about its length times its width, rather than
its length times the square's area (see Settled at review). The scale is
the resolution over 96: `saveas` and `print` without `-r` give 560x420,
`-r192` 1120x840, and `-r0` is the screen's 96.

The PNG is the signature, `IHDR` (8-bit RGB, no interlace), one `IDAT`
and `IEND`, each with its CRC-32 from a table built at compile time. The
`IDAT` data is a zlib stream (`78 01`) of stored deflate blocks of at most
65,535 bytes, the last marked final and possibly empty, each scanline led
by filter type 0, closed by the Adler-32 of the raw rows. The stream is
written straight into the file's buffer, so a PNG costs the pixels and the
file, about six bytes a pixel.

### Bounded work

A plotting call judges its arguments, borrowing them, then counts its
points against the figure's budget, `figure::MAX_POINTS`, 16,777,216,
with the axes' old points left out when hold is off, and only then copies
the data, makes the figure or axes it needs and changes the state. So a
figure's memory is a copy of arrays the user already had, bounded, and a
refusal copies nothing and changes nothing.

A point is what a line's vertex costs: its `x` and `y` in the series, its
pair in the scene and about 14 bytes of SVG. Everything else counts its
bytes of SVG over 16, rounded up, as measured on typical coordinates: a
marker the points of its outline (a circle its centre, a stroke its two
ends) plus five for the rest of its element, so `o` and `.` count 6 (84
and 89 bytes), `+` and `x` 9 (111), `*` 13 (165), `s` and `d` 9 (118),
the triangles 8 (105), `p` 15 (199) and `h` 17 (226); a scatter circle 6
(84 to 92); a bar 9 (122, 138 with a histogram's opacity); each run of a
line 7 for its `<polyline>` element (80, 109 with a dash array); and each
series 9 for its record and legend text, plus one sample of each thing it
draws, for the legend row it may be given. So no figure the budget admits
writes much more than 16 bytes a point, 256 MiB. The runs are counted by
reading the arguments in place; everything else from their sizes.
Measured in the release build at the largest size each admits, every kind
writes 167 to 235 MiB of SVG and peaks at 444 to 611 MB of private
memory, and the plain line of 16,777,193 points, the heaviest, 217 MiB
and 771 MB, as it did before the weights.

Laying a figure out is one pass over its points; the SVG is written with
its text reserved up front from an estimate of each item, so a large
figure never grows it by doubling; the rasterizer's cost is linear in the
points, each segment bounded by the clip. `histogram` counts in one pass
for equal bins and by bisection for given edges, `bar`'s spacing is the
smallest gap between neighbours in the order given (no sort). The PNG's
pixel size goes through `check_shape`, so `-r100000` is `Requested
437500x583333 array exceeds the maximum array size.` before a pixel is
allocated or a file is written; `histogram`'s bin count goes through it
too.

### Output and the viewer

`saveas(fig, name)` takes the format from the extension, `svg` or `png`,
and a name with none (or only its dot) is an error that says how to give
one; `saveas(fig, name, fmt)` takes it from `fmt`, adding the extension to
a name with none. `print` takes `-dpng`, `-dsvg`, `-r<dpi>`, `-f<n>`, a
leading figure number and one file name; with no `-d` the extension
decides, else PNG, and a name with no extension gets the device's. Every
path goes through `Interp::resolve_path`, so against `Interp::cwd`.
`print` with no file name is an error: there is no printer.

The viewer opens only from `main.rs`'s REPL, when standard input is a
terminal: after each entry, `Interp::take_changed_figures` names the
figures the entry changed (any builtin that touched one), each is written
to `splatcrab-<pid>-figure-<n>.svg` in the system's temporary folder,
overwritten on the next change, and handed to `cmd /C start`, `open` or
`xdg-open` through `server::open_browser`, every failure ignored. The files
are left in the temporary folder for the viewer to read. Nothing in the
library opens a viewer, and the protocol, the UI server and scripts never
call `show_figures`.

### Choices where the Scope was silent

- `figure` returns its number only when asked (`n = figure`), so a bare
  `figure` displays nothing; `gcf` always returns it, so a bare `gcf`
  displays `ans = 1`.
- `close(n)` of a figure that is not open is `Invalid figure handle.`;
  `close` with none open does nothing.
- `plot` reads groups of `y`, or `x, y`, each optionally followed by a line
  spec; text where data was expected is `Invalid line specification`, so
  name-value options (`'LineWidth', 2`) are refused rather than ignored.
  A vector against a matrix pairs with the columns of its length, else
  the rows; two matrices must match in size.
- `scatter(x, y, sz, c, 'filled')`: `x` and `y` of one element count,
  whatever their shapes; `sz` one area or one a point, positive; `c` a
  colour letter or an RGB triple.
- `bar(y, width)` when the second argument is a scalar and the first is
  not; one colour letter and `'grouped'` are the only text taken. A
  matrix is grouped, one series per column, the group `width` times the
  spacing wide.
- `histogram(x, n)` bins from the smallest finite value to the largest in
  equal widths, the last bin closed, the width and the edges computed on
  halves so a range wider than the largest double keeps finite edges; a
  single value gets a bin of width 1 about it; `histogram(x, edges)`
  needs two or more increasing edges; with neither, Sturges' rule,
  `ceil(log2(count) + 1)` bins. Non-finite data is
  left out, and with no finite data and no count nothing is drawn.
- `legend` with no argument names the series `data1`, `data2`, ...;
  `legend off` or `hide` removes it; `show` restores the default one.
- `axis` takes `auto`, `tight`, `equal`, `image`, `square`, `normal`,
  `manual`, `on`, `off` and a 4-vector, several in one call; `xlim` and
  `ylim` take `auto`, `manual` and a 2-vector. Limits must increase; an
  infinite end is automatic.
- `grid` and `hold` with no argument toggle; `hold all` is `hold on`.

### Settled in testing

- **Texts the cases pin.** `err_print_huge_resolution.err` is the whole
  of `check_shape`'s text for item 13, `Error: Line 9: Requested
  437500x583333 array exceeds the maximum array size.`, the sizes being
  the 560x420 figure at 100000/96 pixels a figure pixel, rounded. The
  complex refusal is cycle 10's gate text, `Complex values are not
  supported by 'plot'.` Every text listed under the deviations below has
  an `err_*` case: `err_plot_too_many_points`, `err_plot_bad_line_spec`,
  `err_plot_lengths_differ`, `err_bar_text_data`, `err_close_not_open`,
  `err_subplot_index_past_grid`, `err_xlim_decreasing`,
  `err_hold_unknown_option`, `err_saveas_unsupported_format`,
  `err_print_bad_resolution`, `err_print_no_file_name`,
  `err_histogram_zero_bins`, `err_scatter_negative_size`,
  `err_scatter_bad_color` and `err_bar_zero_width`.
- **`gcf_class_double`** prints `double`, from the Scope's `gcf` bullet.
  `limits_axis_clf_run` reads each limit back (`xlim`, `ylim` and `axis`
  with no argument return what the axes shows, as MATLAB's do) and counts
  no `class="axes"` group after `clf`; `grid_on_as_command` finds a
  `class="grid"` line after `grid on` and none after `grid off`, the
  vocabulary above.
- **Limits stay finite and increasing.** A constant at the largest double
  used to widen to `Inf`, and a manual end farther past the data than a
  double resolves (`xlim([1e300 Inf])` over `1:3`) used to put the
  automatic end on top of it; both wrote `NaN` coordinates. The widening
  is clamped to the largest double, the automatic end moves by the data's
  range or by a billionth of the fixed end's magnitude, whichever is
  larger, and `axis equal` keeps an axis as it was when its wider limits
  would overflow (`extreme_limits_stay_numbers`).
- **XML's noncharacters.** U+FFFE and U+FFFF are replaced as a control
  character is, since XML 1.0 forbids them too (`labels_escaped_in_svg`
  checks the escapes and an ESC).
- **Retired case and its replacement**: 04's
  `err_command_hold_unrecognized`, which pinned `hold on` as the
  unrecognized-name error until this cycle, by `hold_on_close_all_commands`
  (item 14); `err_command_format_unrecognized` still pins command syntax
  reaching an unknown name.
- **Cost measured.** The rasterizer is linear in the points with a
  constant of the pixels each one covers: a segment is clipped to its box
  first, so it costs at most the box's diagonal, and a bar its outline.
  In the debug build a million-point `plot` saves as SVG in about 3
  seconds and as PNG in about 1 (11 for a million random points), and a
  million full-height bars take about 100 seconds as PNG (18 in the
  release build), every bar walking its two edges down the whole box.

### Settled at review

- **The budget weighs what each thing writes.** It counted a marker, a
  scatter circle and a bar as one point, though each writes 80 to 250
  bytes of SVG and a scene item: in the release build `plot(1:4e6, 'h')`
  wrote 862 MiB of SVG at a peak of 2.6 GB of private memory, bars at
  the full budget 1.9 GiB at 5.8 GB, and a line broken by a `NaN` at
  every other point 750 MiB at 3.7 GB, one `<polyline>` a point. The
  weights of Bounded work replace the count of one; `plot(1:4e6, 'h')` is
  now refused in a tenth of a second (`err_plot_markers_over_budget`
  refuses a million), and the largest figure of every kind is the one
  measured there.
- **The budget is judged before the copy.** The builtins cloned each
  argument before counting, so memory briefly doubled; they now borrow
  the arguments, count from them in place, and copy inside the closure
  that runs once the budget has passed. A refusal no longer makes a
  figure or an axes either. `plot`'s pairing is a description of its
  lines rather than a list, which for a matrix of millions of columns
  cost 32 bytes a column before the count: refusing `plot(zeros(2, 5e7))`
  peaked at 3.1 GB, and now at the 0.8 GB of the script's own matrix.
- **Wide lines cost their pixels.** Above 1.5 pixels a segment stamped a
  whole square of the width at every step, at a cost of its length times
  the width squared. `Canvas::thick` draws the same union of squares as
  one run across the line for each pixel along it; a unit test compares
  it with the stamping pixel for pixel over 310 segments, seven widths and
  a clip box. In the release build a 10,000-point zigzag printed at
  `-r960`, lines 10 pixels wide, fell from 12.3 to 2.4 seconds, 0.7 of
  them the canvas and the encoder.
- **Histogram edges on halves.** `histogram([1e308 -1e308], 2)` drew
  nothing: the bin width overflowed to `Inf` and the edges were `NaN`.
- **`saveas` with no extension** reported `Unsupported format ''`; it now
  has a text of its own, pinned by `err_saveas_no_extension`.

### Deviations from MATLAB, accepted deliberately

- `gcf` and `n = figure` return the number as a double (the Scope's).
- A complex argument is refused by cycle 10's gate (the Scope's).
- No graphics objects: no builtin returns a handle to a line, an axes or a
  text, and no property can be set.
- Labels are plain text: no TeX interpreter, so `_` and `^` are literal.
- `plot` takes no name-value options; `histogram`'s automatic bins are
  Sturges' rule; the colours of `bar` and `histogram` follow the colour
  order; bars have black edges.
- The tick rule, the layout, the fonts and the bytes of the SVG and the PNG
  are SplatCrab's own, and a line with a `NaN` gap is one `<polyline>` a
  run.
- Texts SplatCrab's own (MATLAB's first for `Invalid figure handle.`):
  `A figure holds at most N points; this plot would take it past that.`,
  `Invalid line specification 'x'.`,
  `Vectors given to 'plot' must be the same length.`,
  `Argument k to 'bar' must be numeric data.`,
  `Invalid figure handle.`,
  `subplot needs positive whole numbers m and n and an index p from 1 to m*n.`,
  `Limits for 'xlim' must be a 2-element vector of increasing numbers.`,
  `Unknown option 'x' for 'hold'.`,
  `Unsupported format 'jpg' for 'saveas'; SplatCrab writes svg and png.`,
  `'saveas' cannot tell the format of 'f': give the file name an extension, .svg or .png, or the format as a third argument.`,
  `Invalid resolution '-rX' for 'print'.`,
  `'print' needs a file name; SplatCrab does not send figures to a printer.`,
  `The bins of 'histogram' must be a positive whole number of bins or increasing bin edges.`,
  `Marker sizes for 'scatter' must be positive numbers, one for all points or one for each.`,
  `Invalid colour for 'scatter': use a letter such as 'r' or an RGB triple in [0, 1].`,
  `The bar width for 'bar' must be a positive number.`

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/12-plotting/`. Expected output is written by hand from MATLAB
semantics wherever feasible; the `.err` substring is always chosen by hand.
Every new error message needs an `err_*` case.

1. `plot(1:3, [1 4 9]); saveas(gcf, 'p1.svg'); s = fileread('p1.svg'); disp(numel(strfind(s, '<polyline'))); disp(~isempty(strfind(s, '>9<')))` → `     1\n   1` (the second line is a logical, four wide since cycle 02) Cases: plot_polyline_and_tick_label, err_plot_complex_input, limits_axis_clf_run, err_plot_lengths_differ, err_xlim_decreasing, err_saveas_unsupported_format, err_saveas_no_extension.
2. `hold on; plot(1:3, 1:3); plot(1:3, 2:4); hold off; saveas(gcf, 'p2.svg'); disp(numel(strfind(fileread('p2.svg'), '<polyline')))` → `     2` Cases: hold_on_two_lines.
3. `subplot(2, 1, 1); plot(1:2); subplot(2, 1, 2); plot(1:3); saveas(gcf, 'p3.svg'); disp(numel(strfind(fileread('p3.svg'), 'class="axes"')))` → `     2` Cases: subplot_two_axes, err_subplot_index_past_grid.
4. `plot(1:2); xlabel('t'); ylabel('y'); title('T'); legend('a'); saveas(gcf, 'p4.svg'); s = fileread('p4.svg'); fprintf('%d %d %d %d\n', ~isempty(strfind(s, '>t<')), ~isempty(strfind(s, '>y<')), ~isempty(strfind(s, '>T<')), ~isempty(strfind(s, '>a<')))` → `1 1 1 1` Cases: labels_title_legend_text, labels_escaped_in_svg.
5. `scatter([1 2 3], [3 1 2]); saveas(gcf, 'p5.svg'); disp(numel(strfind(fileread('p5.svg'), '<circle')))` → `     3` Cases: scatter_circles, err_scatter_negative_size, err_scatter_bad_color.
6. `bar([1 2 3]); saveas(gcf, 'p6.svg'); disp(numel(strfind(fileread('p6.svg'), '<rect class="bar"')))` → `     3` Cases: bar_rects, err_bar_text_data, err_bar_zero_width.
7. `histogram([1 1 2 3 3 3], 3); saveas(gcf, 'p7.svg'); disp(numel(strfind(fileread('p7.svg'), '<rect class="bar"')))` → `     3` Cases: histogram_bins, err_histogram_zero_bins.
8. `plot(1:3, 1:3, 'r--'); saveas(gcf, 'p8.svg'); s = fileread('p8.svg'); fprintf('%d %d\n', ~isempty(strfind(s, 'stroke="#ff0000"')), ~isempty(strfind(s, 'stroke-dasharray')))` → `1 1` Cases: plot_line_spec_red_dashed, err_plot_bad_line_spec.
9. `plot([1 2; 3 4; 5 6]); saveas(gcf, 'p9.svg'); disp(numel(strfind(fileread('p9.svg'), '<polyline')))` → `     2` Cases: plot_matrix_columns.
10. `plot(1:3); print('-dpng', 'p.png'); fid = fopen('p.png'); b = fread(fid, 8); fclose(fid); disp(b')` → `   137    80    78    71    13    10    26    10` Cases: print_png_signature, err_print_bad_resolution, err_print_no_file_name.
11. `figure; figure; disp(gcf)` → `     2`; `close all; figure; disp(gcf)` → `     1` (the figure number, per the `gcf` bullet) Cases: gcf_counts_figures, close_all_restarts_numbering, gcf_class_double, err_close_not_open.
12. `plot(1:1e5); saveas(gcf, 'p12.svg'); disp(numel(strfind(fileread('p12.svg'), '<polyline')))` → `     1`, well under the harness's timeout: a large plot is linear Cases: plot_large_linear, err_plot_too_many_points, err_plot_markers_over_budget, extreme_limits_stay_numbers.
13. `plot(1:3); print('-dpng', '-r100000', 'p13.png')` → a clean error, exit 1, from `check_shape`; no file is left Cases: err_print_huge_resolution.
14. `hold on` and `close all` as commands: `hold on; plot(1:2); plot(2:3); saveas(gcf, 'p14.svg'); disp(numel(strfind(fileread('p14.svg'), '<polyline'))); close all` → `     2` Cases: hold_on_close_all_commands, grid_on_as_command, err_hold_unknown_option.
Each plotting case deletes the files it wrote.

Planning added the viewer rule, the rasterizer and its font, the `gcf`
deviation, the bounded-work and complex rules, command syntax and the
path rule, fixed item 1's logical display, and added items 12 to 14.

## Status

Done (2026-09-29)
