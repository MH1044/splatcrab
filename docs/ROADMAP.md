# Roadmap

One row per module. Each has a spec in `docs/modules/`, and each is built in a
single cycle.

Ordering rationale: lock the infrastructure first, because every later module
registers builtins through it. That work was split in two at planning: the
registry migration and the error-type change each ripple widely, the first
through a 409-line match and the second through 43 lexer assertions, and one
cycle should not carry both. Lock classes and display second, because every
later golden file depends on them and re-blessing expected output later is the
main source of churn. Then indexing forms, which also lands the access-chain
AST and multi-assignment so that cycles 04 through 07 never need to touch the
parser again. Then statements, functions, handles and containers, which
complete the language. Numerics come after the language, plotting and the
environment last.

Between 01b and 02 come three bug-fix cycles, 01c, 01d and 01e. They clear the
"Known bugs" table of `docs/ARCHITECTURE.md`, including the defects found by
the Phase 0 QA pass: 01c the builtin argument forms, 01d numerics and
`printf`, 01e display and the parser. Each adds its row here when its spec is
written. A row that belongs to a roadmap module more naturally stays there,
and the table names the cycle that owns it.

| NN | Module | Goal | Status |
|---|---|---|---|
| 00 | [baseline](modules/00-baseline.md) | The stage-0 interpreter: expressions, matrices, indexing, control flow, 79 builtins | Done (2026-09-27) |
| 01 | [registry-and-builtins](modules/01-registry-and-builtins.md) | The 79 builtins moved out of one 409-line match into a registry that knows `nargout`, plus `tic`/`toc` for 81; arity checks; interpreter on a 256 MB stack | Done (2026-09-27) |
| 01b | [error-reporting](modules/01b-error-reporting.md) | `MError` carrying a line number and every message text, `Error: Line N` in script mode, the three lexer defects found in cycle 0, elementwise `.\`, and a capped range | Done (2026-09-27) |
| 01c | [builtin-arguments](modules/01c-builtin-arguments.md) | The argument forms that cycle 01's arity checks turned into errors (`sort` direction, `find` count, `norm` order, `diag` offset, `num2str` precision, `round` digits), size vectors such as `zeros(size(A))`, `true(n)`, `eps(x)`, `'all'`, and the QA pass's argument defects in the same builtins | Done (2026-09-27) |
| 01d | [numerics-and-printf](modules/01d-numerics-and-printf.md) | The numeric and `printf` defects: `matmul` with `Inf`/`NaN`, a pivot tolerance relative to the norm, unchecked result sizes, `printf` width and precision, colon and `linspace` end points, and the last three inputs that kill the process | Done (2026-09-28) |
| 01e | [display-and-parser](modules/01e-display-and-parser.md) | The display and parser defects: column wrapping, non-finite rows that keep integer columns, MATLAB's empty shapes, chained ranges, readable token names, `break` outside a loop, REPL errors to stderr and an unterminated block at EOF, a skipped byte-order mark and a lenient decode, refusing `NaN` and non-scalars where a logical scalar is required, a recorded error-text policy, and the shared parser and evaluator depth limit that closed the last input able to abort the process | Done (2026-09-28) |
| 02 | [classes-and-display](modules/02-classes-and-display.md) | Logical and char classes as a tag on `Matrix`; class propagation; MATLAB display fidelity | Done (2026-09-28) |
| 03 | [indexing-forms](modules/03-indexing-forms.md) | Logical indexing, element deletion, in-place assignment, access-chain AST, multiple return values | Done (2026-09-28) |
| 04 | [switch-try-commands](modules/04-switch-try-commands.md) | `switch`, `try`/`catch` with an `MException`, `warning`, `assert`, command syntax, block comments | Done (2026-09-28) |
| 05 | [functions-and-scoping](modules/05-functions-and-scoping.md) | User functions in scripts and on a path, frames, `nargin`/`nargout`, `return`, recursion, subfunctions | Done (2026-09-28) |
| 06 | [function-handles](modules/06-function-handles.md) | `@name` and `@(x) body` with capture, `feval`, `arrayfun`, `func2str`, `str2func` | Done (2026-09-28) |
| 07 | [cells-and-structs](modules/07-cells-and-structs.md) | Cell arrays, structs and struct arrays, comma-separated lists, `varargin`/`varargout` | Done (2026-09-28) |
| 08 | [linear-algebra](modules/08-linear-algebra.md) | LU, QR with least squares, Cholesky, eigenvalues, SVD, `rank`, `pinv`, matrix norms | Done (2026-09-28) |
| 09 | [numerics](modules/09-numerics.md) | Polynomials, interpolation, quadrature, root finding, optimisation, `ode45`, filtering, statistics | Done (2026-09-29) |
| 10 | [complex](modules/10-complex.md) | Complex numbers, `fft`, complex eigenvalues. Decided on 2026-09-28: it is built | Done (2026-09-29) |
| 11 | [strings-and-io](modules/11-strings-and-io.md) | String functions, regular expressions, file reading and writing, `save`/`load` | Done (2026-09-29) |
| 12 | [plotting](modules/12-plotting.md) | `plot`, `scatter`, `bar`, `histogram`, `subplot`, labels and legends, SVG and PNG output | Done (2026-09-29) |
| 13 | [environment](modules/13-environment.md) | REPL line editing and history, tab completion, `help`, `which`, `eval`, `format` | Done (2026-09-29) |
| 13b | [three-defects](modules/13b-three-defects.md) | The three Known bugs rows that were real defects: an empty result costs nothing however large a dimension, cells and structs are bounded by bytes as well as elements, and UTF-16 source files are read | Done (2026-09-29) |
| 14 | [nd-arrays](modules/14-nd-arrays.md) | N-D numeric, logical and char arrays: constructors, size queries, `reshape`, indexing with any number of subscripts, element-wise operators with broadcasting and page-by-page display, every other builtin behind a gate | Done (2026-09-30) |
| 14b | [nd-functions](modules/14b-nd-functions.md) | The builtins taught N-D: reductions along any dimension, the element-wise math, `squeeze`, `permute`, `cat`, N-D concatenation, `repmat`, N-D MAT-files | Done (2026-09-30) |
| 14c | [more-nd-builtins](modules/14c-more-nd-builtins.md) | The rest of the everyday library taught N-D: `sort`, `find`, `diff`, `median`, `std`, `var`, `mode`, `fliplr`, `flipud` and `arrayfun`, and new `flip`, `circshift`, `ipermute`, `horzcat`, `vertcat`, `sub2ind` and `ind2sub` | Done (2026-09-30) |
| 15 | [verify-first](modules/15-verify-first.md) | The rows marked verify first, taken to their MathWorks pages: `isequal` of handles, cells and structs, transposed cells and structs, `sum` and `mean` along a dimension of size 1, `any` and `all` past `ndims`, text functions refusing a char matrix; the rest pinned or cited as unsettled | Done (2026-09-30) |
| 16 | [hex-binary-literals](modules/16-hex-binary-literals.md) | `0x2A` and `0b101010` literals with the eight integer-type suffixes and two's complement, each stored as the double its value is | Done (2026-09-30) |
| 17 | [history-bound](modules/17-history-bound.md) | The history file bounded in bytes: an entry of at most 64 KiB is kept, and at most the last 4 MiB of the file is read, and the file compacted to it | Planned |

## The U series: the interface

A second axis, not a continuation of the first. The `NN` numbers above encode
the order the *language* has to be built in, and that order has a written
rationale. The interface is orthogonal to it: it needs no language feature that
does not already exist, and the language needs nothing from it. Numbering it
into the same series would force renumbering around 12 and 13 and would imply a
dependency that is not there.

`GOLDEN_FILTER` is a substring match, so `tests/cases/U0-ui-foundations/` and
friends work with no change to the harness.

The interface is a browser page served by the binary itself over loopback,
using the standard library alone. It is deliberately MATLAB-shaped, and
deliberately better in the places people dislike MATLAB: it starts instantly,
its dark mode is defined once rather than retrofitted, an error takes you to
the line that raised it, plots appear inline rather than in floating windows
that get lost, and the transcript is ordinary selectable text.

| U | Module | Goal | Status |
|---|---|---|---|
| U0 | [ui-foundations](modules/U0-ui-foundations.md) | The evaluation protocol as a stdin/stdout program, covered by the existing golden harness, with no network code at all | Done (2026-09-28) |
| U1 | [ui-server](modules/U1-ui-server.md) | `splatcrab --ui` serves a loopback page that runs a line and shows its exact output; the HTTP bytes are pinned by golden cases | Done (2026-09-28) |
| U2 | [ui-desktop](modules/U2-ui-desktop.md) | Four resizable panes: command window, workspace, file browser and command history, with the palette defined exactly once | Done (2026-09-29) |
| U3 | [ui-editor](modules/U3-ui-editor.md) | An editor with tabs and line numbers, Run and Run Selection, and an error that jumps to its line | Done (2026-09-30) |
| U4 | [ui-figures](modules/U4-ui-figures.md) | Gated on 12 and 13: plots inline, path completion, and `cd` shared between the command window and the file pane | Done (2026-09-30) |

Built between 01e and 02, except U4, which waits for the modules it depends on.
U0 comes first for a reason: it puts the whole protocol behind a stdio program
before any socket exists, so the hard half is covered by ordinary golden cases
and U1 is left as a thin shell over something already proven.

## Dependency policy

**No crate is ever added, for any reason.** Nothing in `[dependencies]`, nothing
in `[dev-dependencies]`, in any module, plotting and the interface included.
The standard library only.

This used to read "no crates until the plotting module", which contradicted
the project's Definition of Done, which
said never. The absolute reading wins and the carve-out is gone.

What that buys, and what it costs. A self-contained binary that builds in
seconds and has no supply chain. In exchange, several things are hand-written
that would otherwise be a line in a manifest: an SVG writer and a store-only
PNG encoder (12), a regular-expression engine and an inflate (11), an HTTP
server and a JSON writer (U1), and raw FFI rather than a crate wherever the
operating system has to be asked something (02, 13).

The cost is unevenly distributed, and the interface pays the most. That is
understood and accepted. If a cycle ever concludes the rule cannot be held, it
stops and asks rather than deciding for itself: adding a dependency is an
escalation, never a cycle decision.
