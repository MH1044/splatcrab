# SplatCrab

A lightweight, open-source MATLAB-compatible numerical language, written in Rust
with no external dependencies.

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
./target/release/splatcrab                # REPL
./target/release/splatcrab examples/demo.m  # run a script
```

## Architecture

```
 .m source ──► lexer.rs ──► parser.rs ──► interp.rs ──► value.rs
              tokens       Stmt / Expr   tree-walking   column-major
                                         evaluator      f64 matrices
```

* **lexer.rs** handles the two MATLAB quirks that live at the token level:
  whitespace separates elements inside `[ ]` (`[1 -2]` vs `[1 - 2]`), and `'`
  is a transpose after a value but a string delimiter otherwise.
* **parser.rs** is a recursive-descent parser with MATLAB's precedence table.
  `end` and bare `:` are only accepted inside an index expression.
* **value.rs** stores matrices column-major like MATLAB, with broadcasting,
  matrix multiply, Gaussian-elimination solve, `inv`, `det`, and MATLAB-style
  display formatting.
* **interp.rs** evaluates statements, resolves `name(args)` as indexing when
  `name` is a variable and as a builtin call otherwise, grows arrays on
  indexed assignment, and holds the builtin library.

## What works today (stage 0)

* Numbers, strings, variables, `ans`
* Matrix literals, ranges `a:b`, `a:s:b`
* Operators: `+ - * / \ ^ .* ./ .^ '` comparison, `& | && || ~`
* Indexing `A(i)`, `A(i,j)`, `A(:,1)`, `A(end)`, `A(end+1) = x` growth
* `if / elseif / else`, `for`, `while`, `break`, `continue`
* ~80 builtins: `zeros ones eye rand linspace size numel length sum prod mean
  max min cumsum abs sqrt exp log sin cos ... mod rem inv det trace diag norm
  dot reshape repmat find sort disp fprintf sprintf num2str error clear who`
* REPL with multi-line continuation for blocks and brackets; `;` suppresses output

## Roadmap

1. **Language** — user functions (`function [a,b] = f(x)`), multiple return
   values, logical indexing `x(x>2)`, element deletion `x(2) = []`, `switch`,
   `try/catch`, anonymous functions `@(x) x.^2`, cell arrays, structs, N-D arrays.
2. **Numerics** — least-squares `\` for non-square systems, `eig`, `svd`, `qr`,
   `lu`, `fft`, `interp1`, `polyfit`, `ode45`, `fzero`, `fminsearch`. Consider
   `faer` or `nalgebra` for the dense kernels once the API surface settles.
3. **Plotting** — `plot`, `scatter`, `histogram`, `subplot`, labels, legends.
   Likely a separate window process talking over a pipe, or SVG/PNG output first.
4. **Environment** — command history, workspace browser, editor, debugger.
5. **Compatibility** — grow the builtin set by running real `.m` files and
   fixing what breaks.
