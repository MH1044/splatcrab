# SplatCrab

A MATLAB-compatible numerical language, written in Rust with zero dependencies.

```
>> A = [1 2; 3 4];
>> A * A

ans =

     7    10
    15    22
```

## Build and run

```
cargo build --release
./target/release/splatcrab                    # REPL
./target/release/splatcrab examples/demo.m    # run a script
```

There are no dependencies, so a clean build takes a few seconds.

## Architecture

```
 .m source ──► lexer.rs ──► parser.rs ──► interp.rs ──► value.rs
              tokens       Stmt / Expr   tree-walking   column-major
                                         evaluator      f64 matrices
                                              │
                                         builtins/    the 81 builtins,
                                                      behind a registry
```

Two MATLAB quirks live in the lexer because they need character-level context:
whitespace separates elements inside brackets, so `[1 -2]` is two elements and
`[1 - 2]` is one; and a quote is a transpose after a value but a string
delimiter otherwise. Matrices are stored column-major, like MATLAB, which is
what makes linear indexing and `reshape` agree with it.

`docs/ARCHITECTURE.md` has the full picture, including the invariants every
change has to preserve.

## What works today

- Numbers, strings, variables, `ans`, comments, line continuation
- Matrix literals, ranges `a:b` and `a:s:b`
- Operators `+ - * / \ ^`, elementwise `.* ./ .^`, transpose, comparisons,
  `& | ~` and short-circuit `&& ||`, with broadcasting
- Indexing `A(i)`, `A(i,j)`, `v(2:4)`, `A(:,1)`, `A(end)`, and growth on
  indexed assignment such as `z(end+1) = x`
- `if` / `elseif` / `else`, `for` over ranges and matrix columns, `while`,
  `break`, `continue`
- Square `A\b`, `inv`, `det`, integer matrix powers
- 81 builtins in a registry, from `zeros` and `linspace` through `sum` and
  `cumsum` to `fprintf`, `sprintf` and `tic`/`toc`. Each is an ordinary
  function with `nargout` in its signature, in `src/builtins/`
- A builtin that produces no value, such as `disp`, is legal as a statement
  and is "Too many output arguments." in an expression; every builtin rejects
  extra arguments with "Too many input arguments."
- A REPL with multi-line continuation, and a script runner

`docs/FEATURES.md` is the full inventory, with the test that proves each entry.

## Not yet

User functions, multiple return values, logical indexing, element deletion,
`switch`, `try`, cells, structs, logical and char classes, complex numbers,
and plotting. `docs/ROADMAP.md` has the order they arrive in, one module at a
time.

## Development

Every module is built in one cycle, and every feature added in a cycle is
covered by a test before that cycle is committed. The testing setup is in
`docs/TESTING.md`.

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

All three must be green before a commit. Tests are golden files under
`tests/cases/`: a `.m` script beside the exact output it must produce.

## Licence

Not yet chosen.
