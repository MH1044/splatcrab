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
matrices, the double, logical and char classes, indexing, control flow,
formatted output and 88 builtins. User functions, cells, structs, complex
numbers and plotting are not there yet. See
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

**The evaluation protocol.** `splatcrab --protocol` is groundwork for a
graphical interface rather than something to type at: it reads one JSON
request per line on stdin and answers each with one JSON line on stdout,
against one session that keeps its variables between requests. The operations
are `eval`, `complete` (is this entry finished?), `workspace` and
`completions`; a failed evaluation or a malformed line is an answer, and the
process exits 0 at end of input. `docs/modules/U0-ui-foundations.md` is
the full description.

```
$ echo '{"id":1,"op":"eval","code":"x = 1 + 2"}' | splatcrab --protocol
{"id":1,"ok":true,"out":"x =\n\n     3\n\n"}
```

**The command window.** `splatcrab --ui` serves a command window in your
browser: type an entry, press Enter, and its output appears exactly as the
terminal would print it. Enter inserts a newline instead while a `for`, an
`if` or a bracket is still open, Shift+Enter always does, and Up and Down
walk the entries you have run. It prints the address it serves and opens it:

```
$ splatcrab --ui
SplatCrab UI: http://127.0.0.1:53817/#3f9c0a5e71d24b88a06e4c19d2f7b350
```

Options: `--port N` serves on port N instead of one the system picks,
`--no-browser` only prints the address, and `--token T` fixes the session
token, for tests. The server listens on `127.0.0.1` alone, never on the
network, and it runs code only for a request that carries the address's
token (the part after `#`, which a browser never sends anywhere by itself)
and names this server in `Host` and `Origin`, so another web page you visit
cannot use it. Stop it with Ctrl+C. `docs/modules/U1-ui-server.md` is the
full description; `splatcrab --http-stdio --port N --token T` answers HTTP
requests from stdin the same way, which is how the tests pin every byte.

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
- Matrix literals, ranges `a:b`, `a:s:b`, and chains of them: `1:2:3:4` reads
  as `(1:2:3):4`, the way MATLAB reads it. Ranges are capped, so `1:1e15` is a
  clean error rather than an allocator abort; one lands exactly on its end
  point and is symmetric about its middle, so `x = 0:0.1:0.3; x(end) == 0.3`
  is `1`. An infinite end point such as `0:Inf` is refused, and an infinite
  *step* follows MATLAB's documented count, so `1:Inf:5` is the one element `1`
- Operators `+ - * / \ ^`, elementwise `.* ./ .\ .^`, transpose, comparisons,
  `& | ~` and short-circuit `&& ||`, with broadcasting. A result too big to
  allocate is a clean error wherever its shape comes from the operands, so
  `ones(1e5,1) + ones(1,1e5)` names the size it was asked for instead of
  aborting the process. A `NaN` is refused wherever a logical is wanted, as
  MATLAB refuses it, and `&&` and `||` need an operand convertible to a
  logical scalar, so `[1 1] && 1` is an error rather than `1`
- Three classes, as in MATLAB: `double`, `logical` and `char`. Arithmetic
  gives a double (`true + true` is `2`, `'a' + 1` is `98`), comparisons and
  `& | ~ && ||` give a logical, and concatenation gives a char if any operand
  is one: `['a' 66]` is `'aB'`. Indexed assignment keeps the left-hand
  side's class, so `s = 'abc'; s(1) = 'X'` is `'Xbc'`, and rearranging a char
  (`fliplr`, `sort`, `reshape`, transpose) keeps it a char. `class`,
  `islogical`, `ischar`, `isnumeric`, `isa`, `logical`, `char` and `double`,
  and `true`, `false`, `any`, `all`, `isnan` and the other predicates return
  logicals
- A char element is a UTF-16 code unit, as in MATLAB, so `length('😀')` is
  `2`, and output decodes it back to UTF-8. On Windows the console is switched
  to UTF-8, so the `×` in a `2×3 char array` header renders
- Indexing `A(i)`, `A(i,j)`, `v(2:4)`, `A(:,1)`, `A(end)`, trailing singleton
  subscripts such as `A(2, 1, 1)`, and growth on indexed assignment such as
  `z(end+1) = x`, which changes the variable in place, so appending in a loop
  stays linear. Logical indexing reads and writes through a mask, `x(x > 0)`
  and `x(isnan(x)) = 0`, selecting what `find(mask)` would. Deletion
  `x(i) = []`, `A(:, j) = []` and `A(i, :) = []` follows MATLAB's shape
  rules. A failed indexed assignment leaves its variable untouched
- Multiple assignment `[a, b] = f(...)`, with `~` to discard an output:
  `[m, i] = max(v)`, `[m, i] = min(v)`, `[s, i] = sort(v)`,
  `[r, c] = size(A)` and `[r, c, v] = find(X)`. The parser also reads the
  access chains cells and structs will use, `c{1}`, `s.a` and `s.(n)`; on a
  matrix they are MATLAB's clean errors
- `if` / `elseif` / `else`, `for` over ranges and matrix columns, `while`,
  `break`, `continue`. A `for` that runs zero times still assigns the empty to
  its loop variable, as MATLAB does, and a `break` with no loop around it is
  an error rather than a silent end to the script
- Nesting is bounded rather than unbounded: 10,000 levels of parentheses,
  brackets, calls, indexes, blocks or chained operators, past which the parser
  and the evaluator both give a clean error. Nothing a user can type aborts
  the process any more
- Square `A\b`, `inv`, `det`, integer matrix powers. The singular test is
  relative to the matrix, so the perfectly conditioned `[1e-15 0; 0 1e-15]` is
  solved rather than written off, and `det` and `\` agree on what singular
  means
- 88 builtins in a registry, from `zeros` and `linspace` through `sum` and
  `cumsum` to `fprintf`, `sprintf`, `class` and `tic`/`toc`. Each is an ordinary
  function with `nargout` in its signature, in `src/builtins/`, and `max`,
  `min`, `sort`, `size` and `find` answer with more than one value when asked
- The argument forms MATLAB code uses: size vectors such as
  `zeros(size(A))` and `reshape(A, [], 2)`, `true(n)` and `eps(x)`,
  `sort(v, 'descend')`, `find(x, n, 'last')`, `norm(v, p)`, `diag(v, k)`,
  `num2str(x, n)`, `round(x, n)` and `round(x, n, 'significant')`,
  `sum(A, 'all')` and `max(A, [], 'all')`, and `dot` of two matrices. A char
  option is never read as a dimension, and a third size other than `1` is the
  clear error "N-D arrays are not supported."
- `fprintf` and `sprintf` with a bounded width and precision, so no format
  specifier can panic or build a pad it cannot afford; `%d` prints an integer
  past `2^63` in full, and `%.Ns` truncates a string before padding it
- A result that would be complex, such as `sqrt(-4)` or `(-8)^(1/3)`, is a
  clean error naming complex numbers rather than a silent `NaN`. Cycle 10
  replaces the error with the value
- A builtin that produces no value, such as `disp`, is legal as a statement
  and is "Too many output arguments." in an expression; every builtin rejects
  extra arguments with "Too many input arguments."
- Display that follows MATLAB's: a logical shows under a `logical` or
  `1×3 logical array` header in four-wide columns, a char keeps its quotes, a
  multi-row char has a `2×3 char array` header, and an empty says what it is
  (`0×3 empty double matrix`). Integers of 1000 and above get wider columns,
  a matrix outside the fixed-point range shares a `1.0e+03 *` scale factor, a
  scalar outside it is `1.2345e+03`, and an exact zero among decimals prints
  as a bare `0`. A matrix too wide for the 80-column window wraps into
  `Columns N through M` blocks, integer columns survive a `NaN` or an `Inf`
  beside them, and `disp([])` prints nothing at all
- Errors that say where they happened: a script prints
  `Error: Line N: <msg>` on stderr and exits 1, reporting the line of the
  statement that raised it, including inside a loop, an `if` body or an
  `elseif` condition. A parse error names the token the way you wrote it, so
  `y = x + ;` reports `unexpected ';' in expression`
- A REPL with multi-line continuation, and a script runner. REPL diagnostics
  go to stderr like a script's, a block left open at end of input is reported
  rather than discarded, and a script file may start with a UTF-8 byte-order
  mark or hold bytes that are not valid UTF-8
- `splatcrab --protocol`, a JSON Lines request loop over one session, with
  `eval`, `complete`, `workspace` and `completions`: the groundwork for the
  interface, with its JSON hand-written rather than taken from a crate
- `splatcrab --ui`, a command window in the browser, served on the loopback
  interface only behind a session token and `Host` and `Origin` checks, over
  HTTP written from the standard library with its size limits enforced
  before anything is buffered

`docs/FEATURES.md` is the full inventory, with the test that proves each entry.
Known differences from MATLAB are listed in `docs/ARCHITECTURE.md`.

## Not yet

User functions, function handles, `switch`, `try`, cells, structs, N-D
arrays, integer classes, complex numbers, and plotting. `docs/ROADMAP.md` has the order they arrive in, one module at a
time.

## How it is built

```
 .m source ──► lexer.rs ──► parser.rs ──► interp.rs ──► value.rs
              tokens       Stmt / Expr   tree-walking   column-major
              + lines      + lines       evaluator      f64 matrices
                                                            + a class tag
                                              │
                                         builtins/    the 88 builtins,
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
prompt must produce, or a `.proto` or `.http` session beside the responses
the protocol or the UI server must give. `docs/TESTING.md` explains the format. Two rules worth
knowing up front: every feature starts as a spec in `docs/modules/`, and the
crate takes no dependencies.

## Licence

MIT. See [LICENSE](LICENSE).

Contributions are accepted under the same licence.

MATLAB is a registered trademark of The MathWorks, Inc. SplatCrab is an
independent project and is not affiliated with or endorsed by The MathWorks.
