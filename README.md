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
matrices, the double, logical and char classes, indexing, control flow with
`switch` and `try`/`catch`, command syntax, user functions in scripts and in
function files on a path, function handles and anonymous functions, cell
arrays and structs, linear algebra, numerics (polynomials, interpolation,
statistics, sets, root finding, minimisation, quadrature and ODEs), formatted
output, complex numbers with `fft`, string functions, regular expressions,
file input and output with `save` and `load`, plotting to SVG and PNG, and
231 builtins. See
[What works today](#what-works-today), [Not yet](#not-yet) and
`docs/ROADMAP.md`.

## Install

You need a Rust toolchain, version 1.85 or newer. If you do not have one,
install it from <https://rustup.rs>. Nothing else is required: the crate has
no dependencies, so a clean build takes a few seconds.

Install it straight from GitHub:

```
cargo install --git https://github.com/MH1044/splatcrab
```

or from a clone of the repository:

```
git clone https://github.com/MH1044/splatcrab
cd splatcrab
cargo install --path .
```

Either puts `splatcrab` in Cargo's bin folder, `~/.cargo/bin`
(`%USERPROFILE%\.cargo\bin` on Windows), which rustup already adds to your
`PATH`. From then on, typing `splatcrab` in any terminal starts the REPL, with
line editing, history and Tab completion; `splatcrab script.m` runs a script,
`splatcrab --help` lists the options and `splatcrab --version` prints the
version. There are no prebuilt binaries yet.

## Use

**REPL.** Run `splatcrab` with no arguments. Type MATLAB expressions at the
`>>` prompt; a line ending in `;` assigns without printing. A `for`, `if`,
`while`, `switch` or `try` block, an unclosed bracket, or an open `%{` block
comment keeps the prompt open until it is closed. Type `exit` or `quit` to leave,
or `exit(n)` to leave with exit code `n`. At a terminal the prompt is a line
editor: the arrow keys move and recall history, Home and End jump, Tab
completes variable, function and file names, and Ctrl-C clears the line. The
history is kept in `~/.splatcrab_history` (or wherever `SPLATCRAB_HISTORY`
points), one entry per line.

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

Output goes to stdout, and a `warning` to stderr. If a statement fails, the
message goes to stderr as `Error: Line N: <message>` and the exit code is 1, so scripts behave well in
shell pipelines and CI. The bundled `examples/demo.m` walks through matrix
arithmetic, indexing, growth on assignment, loops and formatted printing.

**The evaluation protocol.** `splatcrab --protocol` is groundwork for a
graphical interface rather than something to type at: it reads one JSON
request per line on stdin and answers each with one JSON line on stdout,
against one session that keeps its variables between requests. The operations
are `eval` (with the error's stack of frames on request), `complete` (is
this entry finished?), `workspace` (with value previews on request),
`completions`, `files` (one folder under the folder the session started
in), `history` and `history_add` (the command history the terminal keeps),
and `read_file`, `write_file` and `run_file` (one file under that folder,
read, saved or run); a failed evaluation or a malformed line is an answer,
and the process exits 0 at end of input. `docs/modules/U0-ui-foundations.md`,
`docs/modules/U2-ui-desktop.md` and `docs/modules/U3-ui-editor.md` are the
full description.

```
$ echo '{"id":1,"op":"eval","code":"x = 1 + 2"}' | splatcrab --protocol
{"id":1,"ok":true,"out":"x =\n\n     3\n\n"}
```

**The desktop.** `splatcrab --ui` serves a desktop in your browser. In the
middle is the command window: type an entry, press Enter, and its output
appears exactly as the terminal would print it. Enter inserts a newline
instead while a `for`, an `if` or a bracket is still open, Shift+Enter
always does, and Up and Down walk the command history. Around it are a file
browser of the folder the server started in, a workspace listing every
variable with a preview of its value, its size and its class, and the
command history, shared with the terminal, where a click recalls an entry
and a double-click runs it. Above the command window is an editor:
double-click a file in the file browser to open it in a tab, with line
numbers, edit it, save it with Ctrl+S, run it with F5 or run a selection
with F9; an error takes the cursor to the line that raised it, and each
frame of its stack is a link to its file and line. The splitters between
the panes move with the pointer or the arrow keys, and a narrow window
stacks the panes. It prints the address it serves and opens it:

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

- Numbers, strings, variables, `ans`, comments, `%{ ... %}` block comments
  (nestable), line continuation. A `...` separates elements inside brackets
  just as a space does, so `[1 ...` newline `-2]` is two elements
- Command syntax by MATLAB's rule: `clear x y`, `clear all` and `disp hello`
  call the name with char arguments, while `x -1` with `x` a variable stays
  an expression
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
  `[m, i] = max(v)`, `[m, i] = min(v)`, `[s, i] = sort(A)`,
  `[r, c] = size(A)` and `[r, c, v] = find(X)`. On a matrix, `c{1}`, `s.a`
  and `s.(n)` are MATLAB's clean errors
- Cell arrays: `{1, 'two'; [3 4], {5}}` literals, brace and paren indexing,
  growth, deletion, concatenation with `[ ]`, `for` over a cell, `cell`,
  `iscell`, `num2cell`, `cell2mat` and `cellfun`. Structs and struct arrays:
  fields and dynamic fields `s.(name)` created by assignment down any path,
  `p(2).name = 'B'`, `struct`, `fieldnames`, `isfield`, `rmfield`,
  `getfield`, `setfield` and `isstruct`. Comma-separated lists `c{:}` and
  `p.name` spread into calls and brackets, `[a, b] = c{:}`, `deal`, and
  `varargin` and `varargout` with `nargin` and `nargout` counting them.
  MATLAB's displays, `{[1]}    {'ab'}` and `struct with fields:`, bounded for
  any depth of nesting, and a chain nested half a million deep is freed
  without recursion
- `if` / `elseif` / `else`, `for` over ranges and matrix columns, `while`,
  `break`, `continue`. A `for` that runs zero times still assigns the empty to
  its loop variable, as MATLAB does, and a `break` with no loop around it is
  an error rather than a silent end to the script
- `switch` / `case` / `otherwise` on numbers and text, with `case {a, b}`
  lists; `try` / `catch e`, where `e` is a minimal `MException` with
  `e.message`, `e.identifier` and `e.stack`; `error('id:x', fmt, ...)` with MATLAB's
  argument rules, `rethrow`, `lasterr`, `warning`, `assert` and `isequal`
- User functions: local functions at the end of a script, function files
  with subfunctions, and scripts on the path, which run in the caller's
  workspace. Multiple outputs, `nargin`, `nargout`, `return`, a workspace of
  its own for each call, and a recursion limit of 500 that is a clean error.
  A name resolves to a variable, then a function of the running file, then
  one of the script's, then a file in the current folder or on the path,
  then a builtin, so a file shadows a builtin of its name. `addpath`,
  `rmpath`, `exist` and `feval`. An error that leaves a function prints an
  `  in <fn> (line N)` trace after its message
- Function handles: `@name`, bound to the function it names where it is
  made, and anonymous functions `@(x) body`, which capture the variables
  their body reads when they are made and run in a workspace of their own.
  A body that is a single call passes `nargout` on, so `[m, i] = f(v)`
  works for `f = @(v) max(v)`. `feval` of a handle, `arrayfun` with
  uniform output or `'UniformOutput', false`, `func2str`, `str2func`, and
  `class` and `isa` with `'function_handle'`. `size`, `numel`, `isempty`,
  `isa` and the other class and shape queries answer for a handle, an
  `MException`, a cell and a struct, and a binary operator on one of them is
  MATLAB's `Operator '+' is not supported for operands of type 'cell'.`
- Nesting is bounded rather than unbounded: 10,000 levels of parentheses,
  brackets, calls, indexes, blocks or chained operators, past which the parser
  and the evaluator both give a clean error. Nothing a user can type aborts
  the process any more
- Linear algebra on one shared LU with partial pivoting: `A\b` and `b/A`,
  `inv`, `det`, integer matrix powers and `[L, U, P] = lu(A)`. A singular
  square system, `inv` and `A^-1` warn "Matrix is singular to working
  precision." and return a result, `Inf` for `inv([1 2; 2 4])`, rather than
  stopping; the singular test is relative to the matrix, so `det` and `\`
  agree on what singular means. A non-square `A\b` or `b/A` is a
  least-squares solution by column-pivoted Householder QR
- Decompositions: `[Q, R] = qr(A)`, `chol` (and `[R, p] = chol(A)`),
  `eig` and `[V, D] = eig(A)` (Jacobi for a symmetric matrix, Hessenberg QR
  for any other, a complex pair as complex values with complex vectors),
  `svd` and `[U, S, V] = svd(A)` by one-sided Jacobi, and on top of them
  `rank`, `pinv`, `null`, `orth`, `cond` and the matrix `norm(A)`,
  `norm(A, 1)`, `norm(A, Inf)` and `norm(A, 'fro')`. Every iteration has a cap
  and a `NaN` or `Inf` never makes one hang. Also `kron`, `cross`, `triu`,
  `tril` and `magic`
- Numerics: `polyfit`, `polyval`, `roots` (complex roots as complex values),
  `conv`, `deconv` and `filter`;
  `interp1` (linear, nearest, previous, next), `trapz`, `cumtrapz` and
  `diff`; `std`, `var`, `median` and `mode`, column by column on a matrix;
  `factorial`, `nchoosek`, `primes`, `isprime`, `gcd` and `lcm`; `logspace`,
  `meshgrid` and `histc`; and the set functions `unique`, `ismember`,
  `setdiff`, `intersect` and `union`, on arrays and on cells of character
  vectors. `sort` sorts a matrix column by column, or along `dim`
- Solvers that call your function: `fzero` (a sign change, then Brent's
  method), `fminsearch` (Nelder-Mead), `integral` (adaptive Gauss-Kronrod,
  infinite limits allowed) and `ode45` (Dormand-Prince, with `odeset`).
  Each call is counted against the nesting limit, so a solver can call a
  solver, and each has a cap, so no function can make one hang
- 231 builtins in a registry, from `zeros` and `linspace` through `sum` and
  `cumsum` to `fprintf`, `sprintf`, `class`, `feval`, `arrayfun`, `cellfun`,
  `struct` and `tic`/`toc`. Each is an ordinary
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
  past `2^63` in full, and `%.Ns` truncates a string before padding it.
  Since cycle 11 `%x %X %o`, a `*` width or precision, the `#` flag, the
  escapes `\xN`, octal, `\a \b \f \v`, an upper-case `E` for `%E` and
  `%G`, and MATLAB's rule that an invalid conversion ends the output
- Strings (cycle 11): `strcat`, `strsplit`, `strjoin`, `strrep`,
  `strtrim`, `upper`, `lower`, `strcmp`, `strcmpi`, `strncmp`,
  `strncmpi`, `strfind`, `strtok`, `isspace`, `isletter`, `blanks`, and
  the conversions `num2str` (one row per matrix row), `int2str`,
  `mat2str`, `str2double` and `str2num`, taking cells of text where MATLAB
  does. A range between chars, `'a':'e'`, and `diag` of a char keep the
  char class
- Regular expressions: `regexp` with `'match'`, `'tokens'`, `'names'`,
  `'start'`, `'end'`, `'split'` and `'once'`, and `regexprep` with `$N`
  tokens, on an engine of SplatCrab's own that runs in time linear in its
  input, so `(a*)*b` on a long subject returns at once; a backreference is
  refused rather than matched in exponential time
- Files: `input`, `fopen`, `fclose`, `fgetl`, `fgets`, `fprintf(fid, ...)`
  with its byte count, `fread`, `fwrite`, `feof`, `fileread`, `readmatrix`,
  `writematrix`, `csvread`, `csvwrite`, `delete`, and `save` and `load` for
  uncompressed MAT-files of version 5 and for `-ascii` text. `fprintf(2,
  ...)` writes to stderr. `load` reads a MAT-file as untrusted input: every
  size it claims is checked before anything is allocated, so a truncated or
  hostile file is a clean error
- Plotting (cycle 12): `figure`, `gcf`, `close` (`close all`), `clf`,
  `subplot`, `plot` (vectors, one line per matrix column, x-y pairs and
  line specs such as `'r--o'`), `scatter`, `bar` (grouped for a matrix),
  `histogram` (a bin count or edges), `xlabel`, `ylabel`, `title`,
  `legend`, `grid`, `axis`, `xlim`, `ylim` and `hold`, with `hold on`,
  `grid on` and `close all` as commands. `saveas(gcf, 'f.svg')` and
  `print('-dpng', '-r150', 'f.png')` write SVG, or PNG through a rasterizer,
  bitmap font and store-only zlib encoder of SplatCrab's own. At an
  interactive prompt each figure an entry changes opens in the system's
  viewer; a script never opens one. `gcf` is the figure's number, as in
  MATLAB before R2014b, and a figure's work is linear in its points
- Complex numbers (cycle 10): `1i`, `2.5j` and `1e3i` literals, with `i`
  and `j` the imaginary unit unless a variable of the name exists; the
  operators, `'` as the conjugate and `.'` as the plain transpose, matrix
  products and `\` and `/` of complex systems; `real`, `imag`, `conj`,
  `angle`, `abs`, `isreal` and `complex`; `sum`, `prod`, `mean`, `cumsum`,
  `exp`, `sin`, `cos`, `sqrt`, `log`, `log2`, `log10`, `asin`, `acos` and
  `power`, where `sqrt(-4)` is `2i` and `(-8)^(1/3)` is `1 + 1.7321i`;
  complex `eig` and `roots`; `fft` and `ifft` of any length in O(n log n),
  radix-2 or Bluestein. An operation whose imaginary parts are all zero gives
  a real result, as MATLAB's does, and `complex(a, b)` keeps a zero one.
  `==` and `~=` compare both parts, the other comparisons the real parts,
  and `fprintf` prints the real part. Every other builtin refuses a complex
  argument rather than drop the imaginary part
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
  mark, hold bytes that are not valid UTF-8, or be UTF-16, with a byte-order
  mark or without one (pass 13b)
- The environment (cycle 13): `cd`, `pwd`, `ls` and `dir` against the
  interpreter's own current folder, never the process's; `help` (a builtin's
  help line, or a file's leading comment block), `which`, `who` and `whos`
  (with bytes), `format short` and `format long`, `eval`, `evalc` and `run`,
  `datestr`, `now`, `clock`, `pause`, `getenv`, `system` and `version`;
  `exit`, `quit` and `exit(n)` as statements anywhere in a script or at the
  prompt; `clc`, which clears only a real terminal
- `splatcrab --protocol`, a JSON Lines request loop over one session, with
  `eval`, `complete`, `workspace` and `completions`: the groundwork for the
  interface, with its JSON hand-written rather than taken from a crate
- `splatcrab --ui`, a desktop in the browser, served on the loopback
  interface only behind a session token and `Host` and `Origin` checks, over
  HTTP written from the standard library with its size limits enforced
  before anything is buffered, each connection read on a thread of its own
- the desktop's four panes: the command window, the workspace with value
  previews, a file browser confined to the folder the server started in,
  and the command history shared with the terminal, with resizable
  splitters and one palette for light and dark
- the desktop's editor: files in tabs with a line-number gutter, saved
  back with their line ends kept, run whole (F5) or by selection (F9),
  an error taking the cursor to its line and its stack's frames linked to
  their files; unsaved changes asked about in the page; the file
  operations confined to the file root, never through a link out of it,
  and every API call refused unless `Sec-Fetch-Site`, when a browser
  sends it, says the page is the server's own

`docs/FEATURES.md` is the full inventory, with the test that proves each entry.
Known differences from MATLAB are listed in `docs/ARCHITECTURE.md`.

## Not yet

N-D arrays, integer classes, compressed MAT-files, 3-D plots and interaction
with a figure. `docs/ROADMAP.md` has the order they arrive in, one module at
a time.

## How it is built

```
 .m source ──► lexer.rs ──► parser.rs ──► interp.rs ──► value.rs
              tokens       Stmt / Expr   tree-walking   column-major
              + lines      + lines       evaluator      f64 matrices
                                                            + a class tag
                                              │
                                         builtins/    the 231 builtins,
                                                      behind a registry
                                         plot/        figures, SVG, PNG

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
