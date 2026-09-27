# SplatCrab

[![CI](https://github.com/MH1044/splatcrab/actions/workflows/ci.yml/badge.svg)](https://github.com/MH1044/splatcrab/actions/workflows/ci.yml)

A MATLAB-compatible numerical language, written in Rust with zero dependencies.
It runs `.m` scripts, gives you a REPL, and aims to print exactly what MATLAB
prints.

```
>> A = [1 2; 3 4];
>> A * A

ans =

     7    10
    15    22

>> x = A \ [5; 6]

x =

   -4.0000
    4.5000
```

SplatCrab is early software (version 0.1.0). The core language works:
matrices, indexing, control flow, formatted output and 81 builtins. User
functions, cells, structs, complex numbers and plotting are not there yet. See
[What works today](#what-works-today), [Not yet](#not-yet) and
`docs/ROADMAP.md`.

## Install

You need a Rust toolchain, version 1.85 or newer. If you do not have one,
install it from <https://rustup.rs>. Nothing else is required: the crate has
no dependencies, so a clean build takes a few seconds.

**Option 1: install the binary with cargo**

```
cargo install --git https://github.com/MH1044/splatcrab
```

This puts `splatcrab` in `~/.cargo/bin` (`%USERPROFILE%\.cargo\bin` on
Windows), which rustup already adds to your `PATH`.

**Option 2: build from a clone**

```
git clone https://github.com/MH1044/splatcrab
cd splatcrab
cargo build --release
```

The binary is `target/release/splatcrab` (`target\release\splatcrab.exe` on
Windows). Run it from there, or run `cargo install --path .` to put it on your
`PATH`.

There are no prebuilt binaries yet.

## Use

**REPL.** Run `splatcrab` with no arguments. Type MATLAB expressions at the
`>>` prompt; a line ending in `;` assigns without printing. A `for`, `if` or
`while` block, or an unclosed bracket, keeps the prompt open until it is
closed. Type `exit` or `quit` to leave.

```
$ splatcrab
SplatCrab 0.1.0  (type 'exit' to quit)

>> v = 1:5;
>> for k = v
     fprintf('%d squared is %d\n', k, k^2);
   end
1 squared is 1
2 squared is 4
3 squared is 9
4 squared is 16
5 squared is 25
>> exit
```

**Scripts.** Pass a `.m` file:

```
splatcrab examples/demo.m
```

Output goes to stdout. If a statement fails, the message goes to stderr as
`Error: Line N: <message>` and the exit code is 1, so scripts behave well in
shell pipelines and CI. The bundled `examples/demo.m` walks through matrix
arithmetic, indexing, growth on assignment, loops and formatted printing.

**A quick tour.** Paste this into the REPL, or save it as a script:

```matlab
A = [4 -2; 1 1];
b = [2; 3];
x = A \ b                      % solve A x = b
disp(norm(A * x - b) < 1e-10)  % prints 1

v = linspace(0, 1, 5);
fprintf('%.2f ', v .^ 2); fprintf('\n');

z = [];
for k = 1:4
    z(end+1) = k^2;            % grows z one element at a time
end
z
```

## What works today

- Numbers, strings, variables, `ans`, comments, line continuation. A `...`
  separates elements inside brackets just as a space does, so `[1 ...` newline
  `-2]` is two elements
- Matrix literals, ranges `a:b` and `a:s:b`, capped so `1:1e15` is a clean
  error rather than an allocator abort
- Operators `+ - * / \ ^`, elementwise `.* ./ .\ .^`, transpose, comparisons,
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
- Errors that say where they happened: a script prints
  `Error: Line N: <msg>` on stderr and exits 1, reporting the line of the
  statement that raised it, including inside a loop or `if` body
- A REPL with multi-line continuation, and a script runner

`docs/FEATURES.md` is the full inventory, with the test that proves each entry.
Known differences from MATLAB are listed in `docs/ARCHITECTURE.md`.

## Not yet

User functions, multiple return values, logical indexing, element deletion,
`switch`, `try`, cells, structs, logical and char classes, complex numbers,
and plotting. `docs/ROADMAP.md` has the order they arrive in, one module at a
time.

## How it is built

```
 .m source ──► lexer.rs ──► parser.rs ──► interp.rs ──► value.rs
              tokens       Stmt / Expr   tree-walking   column-major
              + lines      + lines       evaluator      f64 matrices
                                              │
                                         builtins/    the 81 builtins,
                                                      behind a registry

                            error.rs: MError, and every message text
```

Two MATLAB quirks live in the lexer because they need character-level context:
whitespace separates elements inside brackets, so `[1 -2]` is two elements and
`[1 - 2]` is one; and a quote is a transpose after a value but a string
delimiter otherwise. Matrices are stored column-major, like MATLAB, which is
what makes linear indexing and `reshape` agree with it. Every error is an
`MError` carrying the line it came from, and every message text is defined in
`error.rs` and nowhere else.

`docs/ARCHITECTURE.md` has the full picture, including the invariants every
change has to preserve.

## Contributing

Bug reports are welcome. The most useful report is the smallest `.m` script
that shows the problem, together with what MATLAB (or GNU Octave) prints for
it.

To work on the code:

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

All three must be green before a commit, and CI runs them on Ubuntu and
Windows. Tests are golden files under `tests/cases/`: a `.m` script beside the
exact output it must produce, or a `.repl` session beside the transcript the
prompt must produce. `docs/TESTING.md` explains the format. Two rules worth
knowing up front: every feature starts as a spec in `docs/modules/`, and the
crate takes no dependencies.

## Licence

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or
  <http://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in SplatCrab by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.

MATLAB is a registered trademark of The MathWorks, Inc. SplatCrab is an
independent project and is not affiliated with or endorsed by The MathWorks.
