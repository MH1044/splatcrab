# 00 — Baseline

## Goal

The stage-0 interpreter: enough of MATLAB to run a useful script. Written
before this process existed, and specified here retroactively so that every
later cycle has a fixed starting point to regress against.

```matlab
A = [1 2; 3 4];
A * A
x = A \ [5; 6];
for k = 1:4
    z(k) = k^2;
end
```

## Scope

- Numbers in every MATLAB form, single- and double-quoted strings with the
  doubled-quote escape, `%` comments, `...` continuation.
- Variables, `ans`, display suppression with `;`.
- Matrix literals with space, comma and newline separators, and nesting.
  Whitespace inside brackets separates elements, so `[1 -2]` is two elements
  and `[1 - 2]` is one.
- Ranges `a:b` and `a:s:b`, including descending and fractional steps.
- Operators `+ - * / \ ^`, elementwise `.* ./ .^`, transpose `'` and `.'`,
  the six comparisons, `& | ~` and short-circuit `&& ||`, with MATLAB
  precedence and scalar and row/column broadcasting.
- Indexing: linear, two-dimensional, vector indices, `A(:,1)`, `A(2,:)`,
  `A(:)`, and `end` anywhere in an index including arithmetic.
- Indexed assignment with growth, in one and two dimensions.
- `if` / `elseif` / `else`, `for` over a range and over matrix columns,
  `while`, `break`, `continue`.
- Column-major double matrices, matrix multiply, square `\` and `/` by
  Gaussian elimination with partial pivoting, `inv`, `det`, integer `A^n`.
- 79 builtins; see `docs/FEATURES.md` for the list. This said 78 until cycle
  01 counted the names in the old `match` and found one more.
- MATLAB-style display and error messages.
- A REPL with block and bracket continuation, and a script runner that exits 1
  on error after flushing stdout.

## Out of scope

Everything in modules 01 through 13. In particular this module has no user
functions, no multiple return values, no logical indexing, no element
deletion, no `switch` or `try`, no cells or structs, no logical or char
classes, no complex numbers and no plotting.

## Design notes

- Matrices are column-major, matching MATLAB, so linear indexing and `reshape`
  agree with it without special cases.
- The two lexer quirks that need character context live in the lexer, not the
  parser: whitespace as a separator inside brackets, and quote as transpose
  versus string delimiter.
- `end` resolves through a stack pushed per index argument.
- A variable shadows a builtin of the same name.
- Errors are `Result<_, String>` throughout; the REPL survives any bad input.

Retrofitted in cycle 0 alongside the harness: all interpreter output moved
behind `Interp::emit` and the `Interp.out` sink so tests can capture it, a
library target was added so tests can drive the interpreter in process, and
`Expr` and `Stmt` gained `PartialEq` so parser tests can compare trees.

## Acceptance tests

21 golden cases in `tests/cases/00-baseline/`.

| Case | Covers |
|---|---|
| `demo_smoke` | A verbatim copy of `examples/demo.m`, the README example |
| `matrix_ops` | Literals, `*`, transpose, `.*`, backslash solve, `inv`, `det` |
| `indexing` | Linear and two-dimensional indexing, `end`, ranges, colon |
| `growth` | `z(end+1)`, two-dimensional growth from a smaller and an empty matrix |
| `control_flow` | `if`/`elseif`/`else`, `for` over a range and over columns, `while`, `break`, `continue` |
| `reductions` | `sum`, `mean`, `prod`, `max`, `min`, `any`, `all`, `cumsum`, `cumprod`, with and without a dimension |
| `fprintf_vector` | Format cycling, width, precision, flags, escapes, `%%`, `%e`, `%g` |
| `display_formats` | Integer, fixed and scientific display; `Inf`, `NaN`, empty |
| `strings` | Both quote styles, the `''` escape, indexing, numeric use, `num2str`, `sprintf` |
| `ranges` | Default, fractional and negative steps; an empty range; `linspace` |
| `logical_ops` | Comparisons, elementwise and short-circuit logicals, negation |
| `builtins_sample` | Constructors, shape queries, elementwise math, linear algebra, `find`, `sort` |
| `who_listing` | The workspace listing. Named `who` until cycle 05, when a
file in the current folder began to shadow a builtin (invariant 4) and the
case's `who` statement resolved to the case itself |
| `rand_shape` | `rand` shape and range only, so the generator can change |
| `err_dims` | Elementwise operation with incompatible sizes |
| `err_matmul` | Matrix multiply with disagreeing inner dimensions |
| `err_undefined` | Undefined name, and stdout flushed before the error |
| `err_singular` | Solving a singular system |
| `err_index` | A zero index |
| `err_unterminated_string` | A string with no closing quote |
| `err_delete_unsupported` | Deletion is not implemented; becomes a passing case in cycle 03 |

Unit tests cover the lexer's token streams, the parser's precedence and tree
shapes, the runtime's broadcasting and solvers with tolerances, and the
formatting helpers.

## Known deviations from MATLAB

Recorded as `% NOTE:` lines in the affected cases rather than silently
blessed. Char arrays display with quotes; an exact zero prints as `0.0000` in a
fixed-point row; empty values print as `[]` rather than a typed header;
integer columns are too narrow above 1000; there is no common scale factor for
non-integer matrices; `det` of an integer matrix prints as an integer; and
`who` prints the typed table that MATLAB calls `whos`. The first six are fixed
in cycle 02 and the last in cycle 13. See `docs/ARCHITECTURE.md`.

## Status

Done (2026-09-27, commit 7791236; harness and specification added in cycle 0)
