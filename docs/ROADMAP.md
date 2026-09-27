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
| 02 | [classes-and-display](modules/02-classes-and-display.md) | Logical and char classes as a tag on `Matrix`; class propagation; MATLAB display fidelity | Planned |
| 03 | [indexing-forms](modules/03-indexing-forms.md) | Logical indexing, element deletion, in-place assignment, access-chain AST, multiple return values | Planned |
| 04 | [switch-try-commands](modules/04-switch-try-commands.md) | `switch`, `try`/`catch` with an error struct, `warning`, `assert`, command syntax, block comments | Planned |
| 05 | [functions-and-scoping](modules/05-functions-and-scoping.md) | User functions in scripts and on a path, frames, `nargin`/`nargout`, `return`, recursion, subfunctions | Planned |
| 06 | [function-handles](modules/06-function-handles.md) | `@name` and `@(x) body` with capture, `feval`, `arrayfun`, `func2str`, `str2func` | Planned |
| 07 | [cells-and-structs](modules/07-cells-and-structs.md) | Cell arrays, structs and struct arrays, comma-separated lists, `varargin`/`varargout` | Planned |
| 08 | [linear-algebra](modules/08-linear-algebra.md) | LU, QR with least squares, Cholesky, eigenvalues, SVD, `rank`, `pinv`, matrix norms | Planned |
| 09 | [numerics](modules/09-numerics.md) | Polynomials, interpolation, quadrature, root finding, optimisation, `ode45`, filtering, statistics | Planned |
| 10 | [complex](modules/10-complex.md) | Complex numbers, `fft`, complex eigenvalues. Optional; decide before cycle 09 | Planned |
| 11 | [strings-and-io](modules/11-strings-and-io.md) | String functions, regular expressions, file reading and writing, `save`/`load` | Planned |
| 12 | [plotting](modules/12-plotting.md) | `plot`, `scatter`, `bar`, `histogram`, `subplot`, labels and legends, SVG and PNG output | Planned |
| 13 | [environment](modules/13-environment.md) | REPL line editing and history, tab completion, `help`, `which`, `eval`, `format` | Planned |

## Dependency policy

No crates and no dev-dependencies until the plotting module. Even there, prefer
a hand-written SVG writer and a store-only PNG encoder over pulling in a
dependency. The point is a self-contained binary that builds in seconds; a
linear algebra crate would be the first thing to weigh, and only once the API
surface has settled.
