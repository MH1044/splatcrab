# SplatCrab Handbook

How to use SplatCrab, for someone who already knows MATLAB.

SplatCrab is a MATLAB-compatible numerical language written in Rust. It runs
`.m` scripts and gives you a REPL. Every value is a matrix stored
column-major, exactly as MATLAB stores it, so linear indexing, `reshape` and
`(:)` agree with MATLAB element for element. A matrix is of class `double`,
`logical` or `char`, as in MATLAB, and may have any number of dimensions.

This handbook is a reference to scan. Every example below was run against the
binary and the output blocks are the bytes it produced. Where SplatCrab
differs from MATLAB, the difference is stated at that point, and
[Differences from MATLAB](#differences-from-matlab) collects them.

`docs/FEATURES.md` is the inventory of what exists, with the test that proves
each entry. `docs/ROADMAP.md` is the order the rest arrives in.

## Contents

- [Running SplatCrab](#running-splatcrab)
- [The language](#the-language)
- [Matrices](#matrices)
- [Operators](#operators)
- [Indexing](#indexing)
- [N-D arrays](#n-d-arrays)
- [Cells and structs](#cells-and-structs)
- [Control flow](#control-flow)
- [Functions](#functions)
- [Builtins](#builtins)
- [Argument forms](#argument-forms)
- [Output and formatting](#output-and-formatting)
- [Errors and exit codes](#errors-and-exit-codes)
- [Differences from MATLAB](#differences-from-matlab)
- [Not yet](#not-yet)

## Running SplatCrab

Build it with `cargo build --release` and the binary is
`target/release/splatcrab` (`target\release\splatcrab.exe` on Windows), or
`cargo install --git https://github.com/MH1044/splatcrab` to put `splatcrab`
on your `PATH`. See `README.md` for the install detail.

**A script.** Put statements in a `.m` file and pass it:

```matlab
A = [1 2; 3 4];
b = [5; 6];
x = A \ b;
fprintf('x = [%.4f %.4f]\n', x);
fprintf('residual = %g\n', norm(A * x - b));
```

```
x = [-4.0000 4.5000]
residual = 0
```

Output goes to stdout. Errors go to stderr as `Error: Line N: <message>` and
the exit code is 1.

**The REPL.** Run `splatcrab` with no arguments. It prints a banner and a
`>>` prompt. A `for`, `if` or `while` block, or an unclosed bracket, keeps
the prompt open until it is closed. `exit` or `quit` leaves, and `exit(n)`
leaves with exit code `n`. At a terminal the prompt is a line editor: the
arrows move and recall history, Home and End jump, Tab completes names
(variables, files on the path, builtins), Ctrl-C clears the line and Ctrl-D
on an empty line leaves. The history is kept in `~/.splatcrab_history`, or
wherever `SPLATCRAB_HISTORY` points, and the browser desktop shares it.
Piped input is read as plain lines. `splatcrab --help` lists the options and
`splatcrab --version` prints the version.

```
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

Errors in the REPL carry no line number, because one line per entry makes it
noise, and the session survives them. Note that REPL diagnostics currently go
to stderr, as in a script, but without a line number: a prompt entry is one
line, so `Line 1:` would be noise.

**The evaluation protocol.** `splatcrab --protocol` is for programs, not
people: it is what the graphical interface will speak. It reads one JSON
object per line on stdin and writes one JSON object per line on stdout, each
answer flushed before the next line is read, all against one session, so a
variable one request assigns is there for the next. Blank lines are skipped.
It writes nothing to stderr and exits 0 at end of input, whatever the
requests did: a failed evaluation and a line that is not a request are both
answers, and the session goes on.

There are thirteen operations. `eval` runs `code` as a REPL entry, possibly
several lines, and answers with the output it would have printed (`out`)
and, if it failed, an `error` holding the REPL's message and the one-based
line within `code`, and with `"stack": true` the error's frames too.
`complete` answers whether `code` is a finished entry or
still inside an open block or bracket, the same test the REPL uses to decide
whether to keep reading. `workspace` lists each variable's name, size and
class, sorted by name, and with `"preview": true` a short `value` of each
too. `completions` lists every variable, function file on the path and
builtin whose name starts with `prefix`. `files` lists one folder under the
file root, the folder the session started in. `history` returns the command
history the terminal's line editor keeps, and `history_add` adds an entry
to it. `read_file` answers the text of one file under the file root,
`write_file` saves a text to one, and `run_file` runs one `.m` file there
as `run` would, answering as `eval` does. `figures` lists the open figures
and those changed since it was last asked, `figure` answers one figure as
the SVG text `saveas` would write, and `cwd` answers the current folder
relative to the file root, or moves it first to the folder `path` names.
Every response starts with the request's `id` (a number or a string, or
`null` when it sent none) and `ok`.

This input, one request per line:

```text
{"id":1,"op":"eval","code":"x = 1 + 2"}
{"id":2,"op":"eval","code":"disp(x * 2)\ny = nosuchname"}
{"id":3,"op":"complete","code":"for k = 1:3"}
{"id":4,"op":"workspace"}
{"id":5,"op":"completions","prefix":"di"}
{"id":6,"op":"fly"}
```

gets exactly these bytes on stdout, and the exit code is 0:

```text
{"id":1,"ok":true,"out":"x =\n\n     3\n\n"}
{"id":2,"ok":false,"out":"     6\n","error":{"message":"Unrecognized function or variable 'nosuchname'.","line":2}}
{"id":3,"ok":true,"complete":false}
{"id":4,"ok":true,"vars":[{"name":"x","size":[1,1],"class":"double"}]}
{"id":5,"ok":true,"items":["diag","diff","dir","disp"]}
{"id":6,"ok":false,"error":{"message":"Unknown operation 'fly'.","line":null}}
```

The `\n` inside a string is JSON's escape for a newline: a request or a
response never spans two lines. An `eval` runs exactly the code it is sent,
so an unclosed `for` is an error answer (`expected 'end' but found end of
input`), not a wait for more; a client asks `complete` first. A line that is
not a usable request is answered with `"ok":false` and a message beginning
`Malformed request:`. The keys, their order and every message are specified
in `docs/modules/U0-ui-foundations.md`. The handbook check runs only
`matlab` examples, so this one is pinned instead by the golden case
`tests/cases/U0-ui-foundations/handbook_protocol_example`, which sends these
six requests and expects these six lines.

**The desktop's operations.** Three operations and one option serve the
browser desktop below. `workspace` with `"preview": true` adds a `value`
after each variable's class: the value itself when it is short (a number, a
small array written `[1,2;3,4]`, a one-row char array quoted as a literal
writes it, a function handle's text) and its size and class otherwise
(`1×11 double`, `1×1 struct`), cut at 80 characters. `files` lists one
folder of the file root, which is the folder the session was started in and
stays there whatever `cd` does: `path` is relative to the root and `""` is
the root itself, folders come first, each file has its size in bytes, and a
path that would leave the root, by a `..`, a link or an absolute name, is
refused. `history` returns the entries of the history file the terminal's
line editor keeps, oldest first, and `history_add` adds one by the
terminal's rule (not a blank one, not a repeat of the newest), so an entry
typed in either place can be recalled in the other after a reload of the
page or a restart of the terminal, since each loads the history once, as
it starts. Started in a folder
named `U2-ui-desktop` that holds `tree/sub/deep.txt`, a file of 3 bytes,
these requests:

```text
{"id":1,"op":"eval","code":"x = 3; name = 'crab';"}
{"id":2,"op":"workspace","preview":true}
{"id":3,"op":"files","path":"tree/sub"}
{"id":4,"op":"files","path":".."}
```

get exactly these answers:

```text
{"id":1,"ok":true,"out":""}
{"id":2,"ok":true,"vars":[{"name":"name","size":[1,4],"class":"char","value":"'crab'"},{"name":"x","size":[1,1],"class":"double","value":"3"}]}
{"id":3,"ok":true,"root":"U2-ui-desktop","path":"tree/sub","entries":[{"name":"deep.txt","dir":false,"size":3}],"truncated":false}
{"id":4,"ok":false,"error":{"message":"Path '..' is outside the file root.","line":null}}
```

The keys, the preview rules, the confinement rule and every message are
specified in `docs/modules/U2-ui-desktop.md`. This example is pinned by the
golden case `tests/cases/U2-ui-desktop/handbook_desktop_example`, which
sends these four requests from that folder and expects these four lines.

**The editor's operations.** Three operations and one option serve the
browser's editor below, each on one file under the file root and confined
to it as `files` is. `read_file` answers a file's `text` exactly, its line
ends and a byte-order mark included, under its `path` normalised; a folder,
a file past 4 MiB or one that is not UTF-8 is refused. `write_file` saves
`text` to `path`, creating or replacing the file, and answers the `size`
it wrote in bytes; it never makes a folder, never writes through a link
that leads out of the root, and refuses a name Windows would read as a
device (`NUL.m`, `con`) or one ending in a dot or a space, on every
platform. A function saved this way runs its new text at its next call,
however soon. `run_file` runs a `.m` file as `run` runs one named by its
full path, in the base workspace, from the file root wherever `cd` has
gone, and answers as `eval` does, except that the error's `line` is `null`,
since no code was submitted, and its `stack` is always there. The stack is
the error's frames, innermost first, each the `file` relative to the root
(or `null` for a function of the submitted code, an anonymous function or
a file outside the root), the `name` the trace prints and the `line`;
`eval` adds it after `line` when the request carries `"stack": true`, and
answers exactly as before without it. Started in a folder holding
`prog/s2.m`, the two lines `q = 1;` and `w = undefined_thing + 1;`, these
requests:

```text
{"id":1,"op":"read_file","path":"prog/s2.m"}
{"id":2,"op":"run_file","path":"prog/s2.m"}
{"id":3,"op":"eval","code":"y = nosuch + 1","stack":true}
```

get exactly these answers:

```text
{"id":1,"ok":true,"path":"prog/s2.m","text":"q = 1;\nw = undefined_thing + 1;\n"}
{"id":2,"ok":false,"out":"","error":{"message":"Unrecognized function or variable 'undefined_thing'.","line":null,"stack":[{"file":"prog/s2.m","name":"s2","line":2}]}}
{"id":3,"ok":false,"out":"","error":{"message":"Unrecognized function or variable 'nosuch'.","line":1,"stack":[]}}
```

The first line of the script ran, so `q` is in the workspace afterwards.
The keys, the order of the checks and every message are specified in
`docs/modules/U3-ui-editor.md`. This example is pinned by the golden case
`tests/cases/U3-ui-editor/handbook_editor_example`, which sends these three
requests from that case's folder and expects these three lines.

**The figures and the current folder.** Three operations serve the
browser's inline figures and its file browser. `figures` answers `open`,
the numbers of the open figures, ascending, and `changed`, those of them
changed since the last `figures` (made current with `figure(n)`, drawn
into, decorated), which asking forgets. `figure` answers figure `n` as
`svg`, the text `saveas(n, 'f.svg')` would write; `n` must be a positive
whole number, a figure that is not open says so, and one whose SVG is
longer than 32 MiB is refused with a word to save it with `saveas`
instead. `cwd` answers the current folder relative to the file root, `""`
at the root itself; with a `path`, judged as `files` judges one, it moves
the current folder there first, exactly as `cd` would. In every client
mode `cd` cannot leave the file root: a `cd` to a folder outside it is
refused, so the command window and the file browser always show the same
folder, while the REPL and scripts are unconfined as before. Started in a
folder named `U4-ui-figures` that holds `tree/a.txt` and
`tree/sub/deep.txt`, these requests:

```text
{"id":1,"op":"eval","code":"figure(2);"}
{"id":2,"op":"figures"}
{"id":3,"op":"cwd","path":"tree"}
{"id":4,"op":"eval","code":"cd ..; cd .."}
```

get exactly these answers:

```text
{"id":1,"ok":true,"out":""}
{"id":2,"ok":true,"open":[2],"changed":[2]}
{"id":3,"ok":true,"cwd":"tree"}
{"id":4,"ok":false,"out":"","error":{"message":"Cannot CD to ..: it is outside the file root.","line":1}}
```

The first `cd ..` of the last request climbs from `tree` back to the root;
the second would leave it, and is refused on line 1 of the code. The keys,
the rules for `n` and every message are specified in
`docs/modules/U4-ui-figures.md`. This example is pinned by the golden case
`tests/cases/U4-ui-figures/handbook_figures_example`, which sends these
four requests from that case's folder and expects these four lines.

**The command window in a browser.** `splatcrab --ui` serves a command
window on your own machine and prints the one line you need:

```text
SplatCrab UI: http://127.0.0.1:52817/#3f9c0a71d2e84b6c95a0f1e7c4d8b263
```

It also opens that address in your default browser, unless you pass
`--no-browser`; `--port N` picks the port instead of letting the system
choose one. The page is a desktop of four panes. In the middle is the
command window, an input with a transcript above it: Enter runs the entry
when it is complete and adds a line when it is not, Up and Down walk the
command history, and each entry's output is shown exactly as the terminal
would print it. A figure the entry made or drew into appears under its
output, labelled `Figure 1` and so on, as the entry left it: a later
entry that draws into the same figure adds a picture of its own under
itself, and the earlier one stays, so the transcript keeps each plot
beside the code that made it. The picture is an image of the figure's SVG,
never put into the page as markup, and a figure too large to show (past
32 MiB of SVG) says so in its place; `saveas` still writes any figure to a
file. Tab in the input completes the name before the cursor, as the
terminal's Tab does: one match replaces it, several are shortened to what
they share and listed under the prompt until the next key. Inside a quoted
string it completes a file or folder name instead, from the current
folder, so `cd('tr` becomes `cd('tree/`. Escape, then Tab, moves the focus
out of the input. On the left the file browser shows the current folder,
one folder at a time, folders first: click a folder to move into it, and
`..`, offered in every folder but the one the server was started in, to go
back up. It is the command window's folder too: `cd tree` in the command
window moves the file browser into `tree`, and a click there moves `pwd`.
`cd` cannot leave the folder the server was started in. On the right the
workspace lists every variable with a preview of its value, its size and
its class, and below it the command history lists what has been run, in
the page and at the terminal alike, oldest first: click an entry to put it
in the input, double-click it to run it. The workspace and the file
browser refresh after every entry.

Above the command window is the editor. Double-click a file in the file
browser to open it in a tab of its own, or choose New for an empty
`untitled.m`. Each tab shows the file's name, a `•` while it has changes
not yet saved, and a close button, and the text has its line numbers
beside it. Tab inserts four spaces, Shift+Tab takes up to four from the
start of each selected line, and Escape then Tab moves the focus out of
the editor. Ctrl+S (Cmd+S on a Mac) saves, keeping the file's line ends;
a new file asks, in the page, for the path to save it at. F5 (or Run)
saves the file if it has changes and runs it, shown in the command window
as `run('prog/s1.m')`, and F9 (or Run Selection) runs the selected text,
or the line the cursor is on, as if typed. When the run fails, the cursor
goes to the line that raised the error, in the tab of the file it is in,
or, when that file is not open in a tab, to the line of the call that led
there in the innermost file that is, the file that was run at the last;
the line is marked beside its number until you next edit. In the
command window every error lists the frames it passed through under its
message, `in helper (line 6)`, and a frame in a file under the folder the
server started in is a link that opens that file at that line. Closing a
tab with unsaved changes asks whether to save, discard them or cancel,
and leaving the page with an unsaved tab brings up the browser's own
warning.

The splitters between the panes move with the pointer or, once focused,
with the arrow keys, and a narrow window stacks the panes in one column,
the command window first and the editor after it. The server runs until
you stop it with Ctrl+C.

The part after `#` is the session token, fresh on every run. The page reads
it from the address and sends it back with every request; nothing without it
is ever run. The server listens on the loopback address `127.0.0.1` only,
so no other machine can reach it, and because any web page you visit could
still send requests to a local port, every request must name this server in
its `Host` header and, when it sends an `Origin`, there too; a request to run
code, list a folder, read or save a file, read or add to the history, see a
figure or move the current folder must also carry the token. A request a
browser marks as made by another site (its `Sec-Fetch-Site` header is
anything but `same-origin`) is refused even with the token; a client that
sends no such header, a script, is unaffected.
Anything else is refused `403 Forbidden` before the interpreter sees
it. The page, its script and its stylesheet need no token: they hold nothing
secret. The page runs only its own script, and shows a figure as an image
made from its SVG by that script, the one kind of image source it admits
beyond its own server.

Under the page, the server speaks the evaluation protocol over HTTP: each
request is one `POST /api` whose body is one protocol request and whose
answer is the protocol's response line. These two requests, to a server
started on port 8123 with the token `test-token` (the first carries it, the
second does not):

```text
POST /api HTTP/1.1
Host: 127.0.0.1:8123
X-SplatCrab-Token: test-token
Content-Type: application/json
Content-Length: 39

{"id":1,"op":"eval","code":"x = 1 + 2"}
POST /api HTTP/1.1
Host: 127.0.0.1:8123
Content-Type: application/json
Content-Length: 35

{"id":2,"op":"eval","code":"y = 4"}
```

get these two responses, whose lines end in CRLF on the wire:

```text
HTTP/1.1 200 OK
Content-Type: application/json
Content-Length: 44
Cache-Control: no-store
X-Content-Type-Options: nosniff
Referrer-Policy: no-referrer
Connection: close

{"id":1,"ok":true,"out":"x =\n\n     3\n\n"}
HTTP/1.1 403 Forbidden
Content-Type: text/plain; charset=utf-8
Content-Length: 13
Cache-Control: no-store
X-Content-Type-Options: nosniff
Referrer-Policy: no-referrer
Connection: close

403 Forbidden
```

The routes, statuses, headers and limits are specified in
`docs/modules/U1-ui-server.md`. `splatcrab --http-stdio --port N --token T`
answers requests read from stdin the same way, with no socket, so this
example is pinned by the golden case
`tests/cases/U1-ui-server/handbook_http_example`, which sends these bytes and
expects these.

## The language

### Numbers

Integer, decimal, leading-dot and exponent forms all lex.

```matlab
x = 12
y = 1.5
z = .5
w = 1e-3
v = 2.5E+2
```

```
x =

    12

y =

    1.5000

z =

    0.5000

w =

   1.0000e-03

v =

   250

```

Every number is an IEEE 754 double. There are no integer classes and no
`single`; complex numbers are the next section. Note `w`: a scalar that is not a whole number
switches to short exponential form below 0.01 and from 1000 up, as MATLAB's
does. [`disp` and automatic display](#disp-and-automatic-display) gives the
rules.

### Complex numbers

Cycle 10. An `i` or a `j` straight after a number makes it imaginary: `1i`,
`2.5j`, `1e3i`. A complex value is still of class `double`, and displays both
parts with four decimals:

```matlab
z = 3 + 4i
w = [1+2i 3-4i; 5 6i]
```

```
z =

   3.0000 + 4.0000i

w =

   1.0000 + 2.0000i   3.0000 - 4.0000i
   5.0000 + 0.0000i   0.0000 + 6.0000i

```

`real imag conj angle abs isreal complex` take them apart. `'` is the
conjugate transpose and `.'` the plain one:

```matlab
z = 3 + 4i;
disp(abs(z)); disp(real(z)); disp(imag(z)); disp(conj(z))
disp([1+2i 3]')
disp([1+2i 3].')
```

```
     5
     3
     4
   3.0000 - 4.0000i
   1.0000 - 2.0000i
   3.0000 + 0.0000i
   1.0000 + 2.0000i
   3.0000 + 0.0000i
```

Every arithmetic operator takes complex operands: the element-wise ones, the
matrix product, and `\`, `/` and an integer matrix power, which solve through
the same LU and least-squares code as a real system. `angle` is the phase, in
`[-pi, pi]`:

```matlab
a = (1+2i) * (3-1i)
b = (5+5i) / (1+2i);
c = [1+1i 2] * [3; 1i];
fprintf('%.4f %.4f\n', real(b), imag(b), real(c), imag(c));
disp(angle(1i) == pi/2)
```

```
a =

   5.0000 + 5.0000i

3.0000 -1.0000
3.0000 5.0000
   1
```

An array is either real or complex as a whole. The rule is MathWorks': an
operation whose imaginary parts all come out zero gives a real result, and
`complex(a, b)` is the one way to keep a zero imaginary part. Indexing and
concatenation follow the same rule, so `z(2)` of `[1+2i 3]` is the real `3`.
An indexed assignment is the exception: `z(1) = 5` changes part of `z` where
it lies and keeps it complex, even if no imaginary part is left, until the
next arithmetic result drops it. `==` and `~=` compare both parts, `<`, `<=`, `>` and `>=` the
real parts only, and the numeric conversions of `fprintf` and `sprintf` print
only the real part:

```matlab
disp(isreal(1i * 0)); disp(isreal(complex(1, 0)))
z = complex(1, 0)
disp((1+2i) == (1+2i)); disp((1+5i) < (2+0i))
fprintf('%d\n', 3+4i);
s = sprintf('%.2f', 2.5-1i)
```

```
   1
   0
z =

   1.0000 + 0.0000i

   1
   1
3
s =

    '2.50'

```

`fft` and `ifft` transform a row along the row and anything else down each
column, for any length in O(n log n): a power of two by radix-2, every other
length by Bluestein's algorithm. The round trip is exact only to rounding, so
print it through `%.4f`:

```matlab
y = fft([1 2 3 4]);
fprintf('%.4f ', abs(y)); fprintf('\n');
x = ifft(fft([1 2 3]));
fprintf('%.4f ', real(x)); fprintf('\n');
```

```
10.0000 2.8284 2.0000 2.8284
1.0000 2.0000 3.0000
```

`i` and `j` are the imaginary unit only while no variable of the name exists,
by the usual resolution order, so a loop counter called `i` shadows it and
`clear i` brings it back:

```matlab
i = 5; disp(i); clear i; disp(i)
```

```
     5
   0.0000 + 1.0000i
```

`sqrt`, `log`, `log2`, `log10`, `asin`, `acos` and a non-integer power of a
negative number give the complex value on the principal branch, and so do
`eig` and `roots` for a complex pair; `fft` and `ifft` transform any length in
O(n log n). A builtin that does not take complex values, such as `sort`,
`max` or `floor`, refuses one rather than drop its imaginary part:

```matlab
sort([1+2i 3])
```

```
Error: Line 1: Complex values are not supported by 'sort'.
```

### Strings

Single quotes make a char array; `''` inside is one quote. Double quotes are
accepted and produce the same char array, where MATLAB has a separate string
class.

```matlab
s = 'hello'
t = 'it''s here'
u = "double"
disp(s)
n = length(s)
c = s(1)
```

```
s =

    'hello'

t =

    'it's here'

u =

    'double'

hello
n =

     5

c =

    'h'

```

Displaying a char shows the quotes, as MATLAB R2018a and later do; `disp` is
bare in both. Chars index, concatenate and compare:

```matlab
a = 'abc' == 'abc'
b = 'abc' == 'abd'
s = 'hello';
c = s(2:4)
d = s + 0
```

```
a =

  1×3 logical array

   1   1   1

b =

  1×3 logical array

   1   1   0

c =

    'ell'

d =

   104   101   108   108   111

```

A comparison gives a `logical` array, shown under its `1×3 logical array`
header. Arithmetic on a char gives numbers, as in MATLAB, while indexing,
indexed assignment and the rearrangement builtins keep it a char. See
[Classes](#classes).

### Variables and `ans`

Names are case-sensitive. An expression statement with no assignment stores
its value in `ans`.

```matlab
a = 3;
a + 4
ans * 2
b = ans
```

```
ans =

     7

ans =

    14

b =

    14

```

### Comments and continuation

`%` starts a comment to end of line. `...` continues a statement onto the
next line.

```matlab
% a whole-line comment
x = 1 + 2;   % a trailing comment
total = 1 + ...
        2 + ...
        3
```

```
total =

     6

```

`...` works straight after a digit, so `a = 1...` continues. Inside brackets
a `...` separates elements just as a space does, so `[1 ...` newline `-2]` is
two elements.

`%{` and `%}`, each alone on its line (spaces around them are allowed),
bracket a block comment. Blocks nest, and a `%{` with anything else on its
line is an ordinary one-line comment. A block left open runs to the end of
the file; at the REPL, the prompt keeps reading until it is closed.

```matlab
%{
disp('hidden')
  %{
  disp('nested, hidden too')
  %}
%}
%{ this line is an ordinary comment
disp('shown')
```

```
shown
```

### Command syntax

A statement that starts with a name that is not a variable, then whitespace,
then a word that is not an operator followed by whitespace, calls that name
with each following word as a char argument. Quotes group words. So
`disp hello` is `disp('hello')` and `clear x y` is `clear('x', 'y')`, while
`x -1` with `x` a variable stays the expression `x - 1`.

```matlab
disp hello
disp 'two words'
x = 1; y = 2;
clear x
who
x = 3;
x -1
```

```
hello
two words
Your variables are:

y

ans =

     2

```

A command ends at a newline, a `,`, a `;` or a `%` outside quotes. Whether a
name is a variable is decided before the script runs, from the names it has
assigned by then, as MATLAB decides it in a file. `hold on`, `grid on`
and `close all` are command syntax for the plotting builtins (see
[Plotting](#plotting)). So are `format long`, `cd folder`, `help sum` and
`which sum` (see [The environment](#the-environment)):

```matlab
format long
disp(pi)
format short
disp(pi)
```

```
   3.141592653589793
    3.1416
```

### Display and `;`

A statement ending in `;` is silent. A newline or a `,` displays the result.

```matlab
a = 5
b = 6;
b
c = 7, d = 8;
```

```
a =

     5

b =

     6

c =

     7

```

### Classes

Every array has a class: `double` for numbers, `logical` for true and false,
`char` for text. `class` names it. Comparisons, `&`, `|`, `~`, `&&` and `||`
give a logical, and so do `true`, `false` and the predicates such as
`isempty` and `isnan`. Arithmetic always gives a double, so `m + 0` turns a
logical into numbers.

```matlab
a = class(5)
b = class('hi')
c = class(3 > 2)
t = true
m = [1 2 3] > 1
n = m + 0
```

```
a =

    'double'

b =

    'char'

c =

    'logical'

t =

  logical

   1

m =

  1×3 logical array

   0   1   1

n =

     0     1     1

```

A logical displays in four-wide columns under a `logical` header; `disp` of a
logical has the same width. `islogical`, `ischar` and `isnumeric` test the
class, and `isa(x, name)` takes a class name or the group `'numeric'`, which
holds `double` alone, since there are no integer classes yet. `logical`,
`double` and `char` convert:

```matlab
disp(islogical(true))
disp(ischar('a'))
disp(isnumeric(true))
disp(isa(2, 'numeric'))
x = logical([2 0 -1])
y = double('AB')
z = char([72 105])
```

```
   1
   1
   0
   1
x =

  1×3 logical array

   1   0   1

y =

    65    66

z =

    'Hi'

```

A char element is one UTF-16 code unit, as in MATLAB, so `length('😀')` is 2
and `double('😀')` is `55357 56832`; output turns the units back into text.
Indexed assignment and growth keep the left-hand side's class, so a character
assigned into a char stays a character, and a concatenation is a char if any
part is one. Unary plus, like any arithmetic, gives a double. A char with
more than one row, or none, displays under a header:

```matlab
s = 'abc';
s(1) = 'X'
t = fliplr(s)
u = [s 'def']
v = s + 1
w = +'a'
c = ['ab'; 'cd']
e = ''
```

```
s =

    'Xbc'

t =

    'cbX'

u =

    'Xbcdef'

v =

    89    99   100

w =

    97

c =

  2×2 char array

    'ab'
    'cd'

e =

  0×0 empty char array

```

`NaN` has no logical value, so converting one is an error, as in MATLAB:

```matlab
b = logical(NaN)
```

```
Error: Line 1: NaN's cannot be converted to logicals.
```

A logical array used as a subscript is a mask: `x(x > 0)` selects the
elements where it is true. See [Logical indexing](#logical-indexing).

## Matrices

### Literals

Spaces or commas separate columns; semicolons or newlines separate rows.

```matlab
A = [1 2 3; 4 5 6]
B = [1, 2, 3]
C = [1 2
     3 4]
```

```
A =

     1     2     3
     4     5     6

B =

     1     2     3

C =

     1     2
     3     4

```

### The whitespace rule

This is the single most surprising thing in the language, and SplatCrab
implements it the way MATLAB does. **Inside brackets**, whitespace is an
element separator, so a sign glued to the following number starts a new
element, while a sign with space on both sides is a binary operator.

```matlab
a = [1 -2]
b = [1 - 2]
c = [1- 2]
d = [1-2]
```

```
a =

     1    -2

b =

    -1

c =

    -1

d =

    -1

```

Only the first is two elements. The same holds for `+`:

```matlab
a = [1 +2]
b = [1 + 2]
c = abs(1 -2)
d = [1, -2]
```

```
a =

     1     2

b =

     3

c =

     1

d =

     1    -2

```

`c` shows that the rule is bracket-only: inside a function call's parentheses
`1 -2` is ordinary subtraction. The safe habits are a comma when you mean two
elements and spaces on both sides when you mean subtraction.

The rule holds after a variable and after a transpose, too:

```matlab
x = 5;
a = [x -1]
b = [x - 1]
c = [x' -1]
```

```
a =

     5    -1

b =

     4

c =

     5    -1

```

### Ranges

`a:b` steps by 1; `a:s:b` steps by `s`. A range that cannot be built is a
clean error rather than an allocator abort, so `1:1e15` fails politely.

```matlab
r = 1:5
s = 0:2:10
t = 5:-1:1
u = 0:0.25:1
v = 5:1
```

```
r =

     1     2     3     4     5

s =

     0     2     4     6     8    10

t =

     5     4     3     2     1

u =

         0    0.2500    0.5000    0.7500    1.0000

v =

  1×0 empty double row vector

```

An exact zero in a fixed-point row prints as a bare `0`, and an empty range
shows its typed header, `1×0 empty double row vector`, as in MATLAB. A
fractional-step range lands exactly on its end point, so
`x = 0:0.1:0.3; x(end) == 0.3` is true, and an infinite end point such as
`0:Inf` is an error.

### Concatenation

Brackets concatenate matrices as well as scalars, provided the dimensions
line up.

```matlab
A = [1 2; 3 4];
B = [5 6; 7 8];
H = [A B]
V = [A; B]
r = [1 2];
w = [r 3 4]
col = [r' r']
```

```
H =

     1     2     5     6
     3     4     7     8

V =

     1     2
     3     4
     5     6
     7     8

w =

     1     2     3     4

col =

     1     1
     2     2

```

## Operators

### Arithmetic

```matlab
a = 7 + 3
b = 7 - 3
c = 7 * 3
d = 7 / 3
e = 2 ^ 10
f = 2 ^ -1
```

```
a =

    10

b =

     4

c =

    21

d =

    2.3333

e =

        1024

f =

    0.5000

```

`e` is twelve wide: from 1000 up, an integer column takes MATLAB's wider
layout.

### Matrix versus elementwise

`*` `/` `\` `^` are matrix operations; the dotted forms `.*` `./` `.\` `.^`
work element by element. `a .\ b` is `b ./ a`.

```matlab
A = [1 2; 3 4];
B = [5 6; 7 8];
P = A * B
E = A .* B
D = A ./ B
L = A .\ B
Q = A .^ 2
M = A ^ 2
```

```
P =

    19    22
    43    50

E =

     5    12
    21    32

D =

    0.2000    0.3333
    0.4286    0.5000

L =

    5.0000    3.0000
    2.3333    2.0000

Q =

     1     4
     9    16

M =

     7    10
    15    22

```

`.\` also parses correctly straight after a number, where the dot could be
read as a decimal point:

```matlab
x = [2 4 8];
a = 2 .\ x
b = x ./ 2
c = 2 ./ x
```

```
a =

     1     2     4

b =

     1     2     4

c =

    1.0000    0.5000    0.2500

```

Backslash solves a linear system, and `/` is its right-hand twin. A square
system goes through an LU factorisation with partial pivoting; a non-square
one is solved in the least-squares sense, since cycle 08 (see
[Linear algebra](#linear-algebra)).

```matlab
A = [4 -2; 1 1];
b = [2; 3];
x = A \ b
y = b' / A'
I = A * inv(A)
```

```
x =

    1.3333
    1.6667

y =

    1.3333    1.6667

I =

    1.0000         0
    0.0000    1.0000

```

`I` shows the difference between an exact zero, which prints as a bare `0`,
and the roundoff residue below it, which is not zero and so keeps `0.0000`.

Transpose is `'`, the conjugate transpose, and `.'` the plain one; they
differ only for [complex numbers](#complex-numbers). The lexer decides between transpose and a string delimiter by what
precedes the quote: after a value it is a transpose, otherwise it opens a
string.

```matlab
A = [1 2; 3 4];
T = A'
U = A.'
v = [1 2 3];
w = v'
```

```
T =

     1     3
     2     4

U =

     1     3
     2     4

w =

     1
     2
     3

```

### Comparison

`==  ~=  <  <=  >  >=` compare element by element and return a `logical`
array, displayed under its `logical array` header in four-wide columns. Used
as a subscript, one is a mask, `x(x > 0)`; see
[Logical indexing](#logical-indexing).

```matlab
v = [1 5 3];
a = v > 2
b = v == 3
c = v ~= 3
d = v <= 3
```

```
a =

  1×3 logical array

   0   1   1

b =

  1×3 logical array

   0   0   1

c =

  1×3 logical array

   1   1   0

d =

  1×3 logical array

   1   0   1

```

### Logical and short-circuit

`&` `|` `~` are elementwise. `&&` and `||` short-circuit and expect scalars.

```matlab
a = [1 0 1] & [1 1 0]
b = [1 0 1] | [0 0 0]
c = ~[1 0 2]
d = (1 > 0) && (2 > 1)
e = (1 > 2) || (3 > 2)
```

```
a =

  1×3 logical array

   1   0   0

b =

  1×3 logical array

   1   0   1

c =

  1×3 logical array

   0   1   0

d =

  logical

   1

e =

  logical

   1

```

All five give a logical. `&&` and `||` refuse a non-scalar or empty operand,
and every logical operator refuses `NaN`, as MATLAB does. See
[Logical values](#logical-values).

### Precedence

Highest to lowest: transpose `'`, then `^` and `.^`, then unary `-` and `~`,
then `* / \ .* ./ .\`, then `+ -`, then `:`, then the comparisons, then `&`,
then `|`, then `&&`, then `||`. `^` is left-associative.

```matlab
a = 2 + 3 * 4
b = (2 + 3) * 4
c = -2 ^ 2
d = 2 ^ 3 ^ 2
e = 1:3 + 1
f = (1:3) + 1
g = ~0 == 1
h = 1 < 2 & 2 < 3
```

```
a =

    14

b =

    20

c =

    -4

d =

    64

e =

     1     2     3     4

f =

     2     3     4

g =

  logical

   1

h =

  logical

   1

```

`e` is the one to watch: `:` binds looser than `+`, so `1:3 + 1` is `1:4`.

### Broadcasting

A scalar expands against anything. A row and a column expand against each
other, as in MATLAB R2016b and later.

```matlab
A = [1 2 3; 4 5 6];
a = A + 10
b = A .* [1 2 3]
c = A + [10; 20]
d = [1 2 3] + [10; 20]
```

```
a =

    11    12    13
    14    15    16

b =

     1     4     9
     4    10    18

c =

    11    12    13
    24    25    26

d =

    11    12    13
    21    22    23

```

## Indexing

### Linear

One subscript walks the matrix in column-major order.

```matlab
A = [1 2; 3 4];
a = A(1)
b = A(2)
c = A(3)
d = A(4)
e = A(:)'
```

```
a =

     1

b =

     3

c =

     2

d =

     4

e =

     1     3     2     4

```

### Two-dimensional and colon

```matlab
A = [1 2 3; 4 5 6];
a = A(2,3)
b = A(:,2)
c = A(1,:)
d = A(:,[1 3])
e = A(:, 2:3)
```

```
a =

     6

b =

     2
     5

c =

     1     2     3

d =

     1     3
     4     6

e =

     2     3
     5     6

```

### `end`

`end` is the last index of the dimension it appears in, and takes part in
arithmetic.

```matlab
v = 10:10:50;
a = v(end)
b = v(end-1)
c = v(2:end)
A = [1 2 3; 4 5 6];
d = A(end, end)
e = A(end)
f = A(1, end-1)
```

```
a =

    50

b =

    40

c =

    20    30    40    50

d =

     6

e =

     6

f =

     2

```

### Growth on assignment

Assigning past the end grows the array, zero-filling the gap.

```matlab
v = [1 2 3];
v(2) = 20
v(6) = 60
A = [1 2; 3 4];
A(3,3) = 9
z = [];
for k = 1:4
    z(end+1) = k^2;
end
z
```

```
v =

     1    20     3

v =

     1    20     3     0     0    60

A =

     1     2     0
     3     4     0
     0     0     9

z =

     1     4     9    16

```

A slice assignment takes a matching block, or a scalar that fills it:

```matlab
A = [1 2 3; 4 5 6];
A(:,2) = [0; 0]
A(1,:) = 7
```

```
A =

     1     0     3
     4     0     6

A =

     7     7     7
     4     0     6

```

Appending with `z(end+1) = k` in a loop is amortised: the array grows in
place rather than being copied on every step. An indexed assignment checks
every subscript, the element count and any growth before it changes anything,
so one that fails leaves the variable exactly as it was.

### Logical indexing

A `logical` subscript is a mask: it selects the elements where it is `true`,
for reading and for assignment alike.

```matlab
x = [5 3 8 1];
a = x(x > 2)
x(x > 4) = 0
A = [1 2 3; 4 5 6; 7 8 9];
b = A(A > 5)'
```

```
a =

     5     3     8

x =

     0     3     0     1

b =

     7     8     6     9

```

A mask selects exactly what `find(mask)` would, and that fixes the result's
shape: a mask indexing a vector keeps the vector's orientation, and any other
mask gives a column, as `A(A > 5)` does above. A mask shorter than the array
selects only among the elements it covers. A `true` past the end grows the
array on assignment, as a numeric index would:

```matlab
x = [10 20 30];
a = x(logical([1 0]))
A = [1 2; 3 4];
b = A(logical([1 0 0 1]))
c = A(logical([1 0; 0 1]))
y = [1 2];
y(logical([0 0 1])) = 9
```

```
a =

    10

b =

     1     4

c =

     1
     4

y =

     1     2     9

```

On a read, a `true` past the end is the ordinary out-of-bounds error:

```matlab
x = [10 20 30];
x(logical([0 0 0 1]))
```

```
Error: Line 2: Index exceeds the number of array elements. Index must not exceed 3.
```

A double of ones and zeros is not a mask: `x([1 1 1])` is the first element
three times. Convert it with `logical` first.

### Deletion

Assigning the empty literal `[]` to an indexed target deletes those elements.
With one subscript, a column vector stays a column and anything else, a
matrix included, becomes a row of what is left, in column-major order. With
two, one of them must be `:` (or select the whole dimension), and whole rows
or columns go:

```matlab
x = 1:5;
x(2) = []
x([1 end]) = []
A = [1 2 3; 4 5 6];
A(:, 2) = []
A(1, :) = []
B = [1 2; 3 4];
B(2) = []
```

```
x =

     1     3     4     5

x =

     3     4

A =

     1     3
     4     6

A =

     4     6

B =

     1     2     4

```

A mask deletes too: `x(x < 0) = []` removes the negative elements. Deleting a
single element of a matrix would leave a hole, so it is an error, as in
MATLAB:

```matlab
A = [1 2; 3 4];
A(1, 2) = []
```

```
Error: Line 2: A null assignment can have only one non-colon index.
```

Only the literal `[]` deletes. `e = []; x(2) = e` is an ordinary assignment of
an empty, and fails the element count.

### Trailing singleton subscripts

A matrix has as many trailing dimensions of size 1 as you care to name, so a
third (or later) subscript of `1` is accepted, and `end` there is `1`:

```matlab
A = [1 2; 3 4];
a = A(2, 1, 1)
b = A(:, :, 1)
A(1, 2, 1) = 9
c = A(1, 1, end)
```

```
a =

     3

b =

     1     2
     3     4

A =

     1     9
     3     4

c =

     1

```

Anything past 1 there is out of bounds when you read it:

```matlab
A = [1 2; 3 4];
A(1, 1, 2)
```

```
Error: Line 2: Index in position 3 exceeds array bounds. Index must not exceed 1.
```

Assigning there grows the matrix into a second page, as the next section
shows.

### Brace and dot on a matrix

`x{1}`, `x.a` and `x.(name)` parse, but a matrix has no cells and no fields, so
at run time each is MATLAB's own error:

```matlab
x = [1 2];
x{1}
```

```
Error: Line 2: Brace indexing is not supported for variables of this type.
```

```matlab
x = [1 2];
x.a
```

```
Error: Line 2: Dot indexing is not supported for variables of this type.
```

Cell arrays and structs, which do support them, come after N-D arrays.
Assigning through a brace or a field into a matrix is MATLAB's assignment
form of the same error, `Unable to perform assignment because dot indexing
is not supported for variables of this type.`

## N-D arrays

A numeric, logical or char array can have any number of dimensions. The
constructors `zeros`, `ones`, `rand`, `NaN`, `Inf`, `true` and `false` take
three or more sizes or a size vector of any length, `zeros(2, 3, 4)` and
`ones([2 2 2])`, with trailing sizes of 1 dropped as MATLAB drops them;
`reshape` takes any number of sizes and one `[]`; and an assignment past
the end grows an array into a new page. A named display
shows an N-D array page by page, each page headed by its index and laid
out as that page alone would be:

```matlab
A = reshape(1:8, 2, 2, 2)
p = A(:, :, 2)
B = zeros(2, 2);
B(:, :, 2) = [1 2; 3 4];
disp(size(B))
```

```
A(:,:,1) =

     1     3
     2     4


A(:,:,2) =

     5     7
     6     8

p =

     5     7
     6     8

     2     2     2
```

The shape queries see every dimension. `size` with fewer outputs than
dimensions folds the rest into its last output and gives `1` for any
output past them, as does `size(A, k)` past `ndims(A)`. `ndims` is 2 for
every 2-D value, a cell or a struct included, and `isvector` and
`isscalar` are false for any N-D array:

```matlab
A = zeros(2, 3, 4);
disp(size(A))
[r, c] = size(A);
disp([r c])
disp([ndims(A) numel(A) size(A, 5)])
disp([ndims(5) ndims({}) isvector(zeros(1, 1, 3))])
```

```
     2     3     4
     2    12
     3    24     1
     2     2     0
```

An index takes any number of subscripts. With fewer than the array has
dimensions, the last one indexes every remaining dimension folded into
one, so `A(2, 7)` of a 2x3x4 is element (2, 7) of it as a 2x12, and `end`
in the last position is the product of the dimensions it spans. A single
subscript is linear over every element, and a read keeps one dimension
per subscript. Deleting with one subscript that is not `:` removes those
positions along its dimension:

```matlab
A = reshape(1:24, 2, 3, 4);
disp(A(2, 7))
disp(A(1, end))
v = A(1, 2, :);
disp(v(:)')
disp(size(A(1, :, :)))
A(:, :, 2) = [];
disp(size(A))
```

```
    14
    23
     3     9    15    21
     1     3     4
     2     3     3
```

The element-wise operators broadcast across every dimension: two
dimensions agree when they are equal or one of them is 1, and a dimension
past an operand's last is 1. `disp` writes the pages under headers of
`(:,:,k) =`, and an empty N-D array says what it is:

```matlab
A = reshape(1:8, 2, 2, 2);
D = A - [10; 20];
disp(D(:)')
L = A > 4;
disp(class(L))
disp(A)
E = zeros(2, 0, 3)
```

```
    -9   -18    -7   -16    -5   -14    -3   -12
logical
(:,:,1) =

     1     3
     2     4


(:,:,2) =

     5     7
     6     8

E =

  2×0×3 empty double array

```

`for` over an N-D array iterates the columns of its 2-D fold, each a column
of the array's rows:

```matlab
for col = reshape(1:6, 2, 1, 3)
    disp(col')
end
```

```
     1     2
     3     4
     5     6
```

The reductions work along any dimension. With no dimension, `sum`,
`prod`, `mean`, `any`, `all`, `max`, `min`, `cumsum` and `cumprod` run
along the first dimension whose size is not 1, so `sum(ones(1, 1, 3))` is
3; along a dimension that dimension becomes 1 and every other stays;
along one past the array's the array is handed back as it is, as it is for
a matrix, so `sum(A, 5)` is `A`, and `sum` and `mean` hand it back along a
dimension of size 1 too, where every other reduction reduces each element
on its own. `max(A, [], 3)` gives the index along the
third dimension as its second output, and `max(A, B, 3)` of two arrays is
refused, since a dimension goes with one array. The element-wise math,
`abs`, `sqrt`, `floor`, `round`, `isnan` and the rest, keeps every
dimension, and `mod`, `rem`, `atan2`, `hypot`, `power` and `complex`
broadcast across them. `permute` rearranges the dimensions, dimension `i`
of the result being dimension `dimorder(i)` of the argument, `squeeze`
removes the dimensions of length 1, and `cat` joins arrays along any
dimension, by the rule a bracket with an N-D operand follows too, so
`[A, A]` joins along the second:

```matlab
A = reshape(1:24, 2, 3, 4);
S = sum(A, 3)
P = permute(A, [3 1 2]);
disp(size(P))
disp(P(:, 1, 1)')
C = cat(3, [1 2; 3 4], [5 6; 7 8])
disp(size([A, A]))
```

```
S =

    40    48    56
    44    52    60

     4     2     3
     1     7    13    19
C(:,:,1) =

     1     2
     3     4


C(:,:,2) =

     5     6
     7     8

     2     6     4
```

Every other dimension of a concatenation must agree, a dimension past an
array's last being 1, or it is `Dimensions of arrays being concatenated
are not consistent.`; an empty array beside a nonempty one is left out.
A bracket of matrices alone keeps the rule it always had, so `[1:0]` and
`[zeros(1, 0), zeros(0, 1)]` are still the 0x0 `[]`, where `cat(2, 1:0)`
is 1x0 and `cat(2, zeros(1, 0), zeros(0, 1))` an error.
`repmat` tiles along every dimension it is given, `repmat(A, 1, 1, 2)` of
a 2x3x4 being 2x3x8, and `save` and `load` keep an N-D array in a MAT-file
with every dimension, in a variable, a cell or a field.

Search, sort and the statistics work along any dimension too. `sort`
sorts each slice along the first dimension whose size is not 1, or along
`dim`, stably, and its second output is each element's position along
it; `find` of an N-D array is a column of linear indices, and with two
outputs the second is the linear index over every dimension past the
first, so `A(r, c)` is the element found; `median`, `std`, `var` and
`mode` reduce along a dimension as `sum` does; and `diff` works along any
dimension, its result empty along one past the array's. As their pages
say, `find(0)` is `[]`, and past the array's dimensions `std` and `var`
are zeros, a `NaN` included, where `median` and `mode` hand the array
back:

```matlab
A = reshape(1:24, 2, 3, 4);
B = sort(-A, 3);
disp(B(:, :, 1))
[r, c] = find(A == 14);
disp([r c])
k = find(A > 22)
M = median(A, 3)
disp(size(diff(A, 1, 3)))
disp([var(NaN, 0, 3) size(find(0))])
```

```
   -19   -21   -23
   -20   -22   -24
     2     7
k =

    23
    24

M =

    10    12    14
    11    13    15

     2     3     3
     0     0     0
```

`flip` reverses the order of the elements along the first dimension whose
size is not 1, or along `dim`, and `fliplr` and `flipud` flip each page on
its own; `circshift` shifts the elements circularly, a vector of shifts
moving each dimension by its own; `ipermute` undoes `permute`; `horzcat`
and `vertcat` are `cat` along the second and the first dimension, with
`cat`'s rule for empties rather than the brackets'; and `sub2ind` and
`ind2sub` convert between subscripts and linear indices as indexing does,
the last subscript folding the dimensions after it. `arrayfun` takes
arrays of any one size, its uniform result of that size:

```matlab
A = reshape(1:24, 2, 3, 4);
F = flip(A, 3);
disp(F(:, :, 1))
disp(circshift(1:5, 2))
C = circshift(A, 1, 3);
disp(isequal(C(:, :, 2), A(:, :, 1)))
P = permute(A, [3 1 2]);
disp(isequal(ipermute(P, [3 1 2]), A))
disp([size(horzcat(zeros(1, 0), zeros(1, 0))) size([zeros(1, 0), zeros(1, 0)])])
i = sub2ind(size(A), 2, 1, 2)
[r, c, p] = ind2sub(size(A), 14);
disp([r c p])
```

```
    19    21    23
    20    22    24
     4     5     1     2     3
   1
   1
     1     0     0     0
i =

     8

     2     1     3
```

The builtins that take an N-D array are the constructors, the shape and
class queries, `ndims`, `reshape`, `isequal`, `disp`, `double`,
`logical`, `char`, `fprintf`, `sprintf`, `feval`, `deal`, the reductions,
the element-wise math, `squeeze`, `permute`, `cat`, `repmat`, `sort`,
`find`, `diff`, `median`, `std`, `var`, `mode`, `fliplr`, `flipud`,
`arrayfun`, `flip`, `circshift`, `ipermute`, `horzcat`, `vertcat`,
`sub2ind` and `ind2sub`. Every other builtin, `num2str`, `mat2str`, the
strings, the sets, the linear algebra and the rest until a later cycle
([Not yet](#not-yet)), refuses one by name rather than read its first
page alone, the matrix operators refuse one where they are not
element-wise, and so do `'`, `.'` and `save -ascii`, whose text has rows
alone:

```matlab
A = zeros(2, 2, 2);
s = num2str(A)
```

```
Error: Line 2: N-D arrays are not supported by 'num2str'.
```

`A * 2`, `A / 2` and `2 \ A` are element-wise, as they are for a matrix;
`A * B` of two arrays is `Matrix operations are not defined for N-D
arrays.` and `A'` is `Transpose is not defined for N-D arrays.`, for which
`permute(A, [2 1 3])` is the page-by-page `.'`, the transpose without the
conjugate, so of a complex array it is not the page-by-page `'`. Cells
and structs
stay 2-D, though an element or a field may hold an N-D array, which a cell's
display summarises as `{2×3×4 double}` and a struct's as `[2×3×4 double]`.
A builtin that reads a cell's elements judges an N-D one itself: `unique`
and the other set functions refuse it, `str2double` reads it as `NaN`, and
`strcmp` and `strcmpi` compare it by every dimension, so the same
characters in another shape are not the same text.

## Cells and structs

### Cell arrays

A cell array holds values of any kind, stored column-major like a matrix.
`{...}` builds one, with a bracket's separators: a comma or whitespace
between elements, a semicolon or a newline between rows. Braces read an
element's contents, parentheses index the cell itself and give a cell, and a
chain goes on into the contents. A brace assignment past the end grows the
cell with `[]` elements, and `c(k) = []` deletes:

```matlab
c = {1, 'two', [3 4]}
disp(c{2})
disp(c{3}(2))
d = c(2:3);
disp(class(d))
c{5} = 'x';
disp(size(c))
c(2) = [];
disp(numel(c))
e = cell(2, 2)
```

```
c =

  1×3 cell array

    {[1]}    {'two'}    {1×2 double}

two
     4
cell
     1     5
     4
e =

  2×2 cell array

    {0×0 double}    {0×0 double}
    {0×0 double}    {0×0 double}

```

Each element displays on one line: a scalar in brackets, a char row
quoted, a handle as its text, and anything else as its size and class. A
cell inside a cell is `{1×2 cell}` and is never expanded, so a cell nested
any depth displays at once. A column is as wide as its widest element; a
number is padded inside its brackets and anything else before its closing
brace. `[{1}, 2]` is a cell, the number becoming one element of it, and `{c}`
of a cell nests it. A handle may be an element, `{@(x) x + 1, 2}`, and
`c{1}(3)` calls it. A binary operator on a cell is MATLAB's refusal:

```matlab
c = {1};
c + 1
```

```
Error: Line 2: Operator '+' is not supported for operands of type 'cell'.
```

`c'`, `c.'` and `transpose(c)` interchange the row and column of every
element, each element itself unchanged. `isequal` compares two cells
element by element, by the rules it compares arrays by: the same size,
every pair of elements equal, the class not compared and a `NaN` equal to
nothing, however deeply the cells nest:

```matlab
c = {1, 'ab'; [3 4], 2};
d = c';
disp(size(d))
disp(d{1, 2})
disp(d{2, 1})
disp(isequal(c', c.'))
disp(isequal({1, 'a'}, {1, 'a'}))
disp(isequal({'a'}, {97}))
disp(isequal({1, 2}, {1; 2}))
disp(isequal({NaN}, {NaN}))
```

```
     2     2
     3     4
ab
   1
   1
   1
   0
   0
```

### Structs

Assigning a field makes a struct, and a field of a field a struct inside
it; `s.(name)` is the field a char names. `struct('a', 1, ...)` builds one
directly:

```matlab
s.name = 'Ada';
s.born = 1815;
s.inner.v = 3;
s
n = 'born';
disp(s.(n))
s.inner.v = s.inner.v + 1;
disp(s.inner.v)
disp(fieldnames(s))
t = rmfield(s, 'inner');
disp(isfield(t, {'name', 'inner'}))
u = struct('x', 5, 'y', [1 2])
```

```
s =

  struct with fields:

     name: 'Ada'
     born: 1815
    inner: [1×1 struct]

        1815
     4
    {'name' }
    {'born' }
    {'inner'}
   1   0
u =

  struct with fields:

    x: 5
    y: [1 2]

```

The field names are right-aligned and each value is summarised on one line,
as a cell's elements are. `getfield(s, 'a')` and `setfield(s, 'a', v)` are
the function forms; `setfield` returns a copy and leaves `s` alone.
`isstruct` and `iscell` test the kind. A struct is not an array of numbers,
so a binary operator refuses it in the same sentence as a cell's:

```matlab
s.a = 1;
s * 2
```

```
Error: Line 2: Operator '*' is not supported for operands of type 'struct'.
```

`isequal` of two structs needs the same fields, in any order, and every
field equal; a struct never equals a cell or an array:

```matlab
s.a = 1;
s.b = 'x';
t.b = 'x';
t.a = 1;
disp(isequal(s, t))
t.b = 'y';
disp(isequal(s, t))
disp(isequal(s, {1}))
```

```
   1
   0
   0
```

### Struct arrays and cs-lists

`p(2).name = 'B'` grows a struct array, every element having every field.
`p.name` of a struct array and `c{:}` of a cell are comma-separated lists:
in a call's arguments, a bracket or a brace they become one value per
element, `[a, b] = c{:}` assigns them in order, and anywhere one value is
needed a list of any other length is MATLAB's error. A struct array
transposes as a cell does, `p'` of a 1x2 being 2x1 with the same fields.
`struct` with cell values makes a struct array of the cells' size:

```matlab
p(1).name = 'A';
p(2).name = 'B';
p
disp([p.name])
disp(size(p'))
q = struct('v', {10, 20, 30});
disp(size(q))
disp(sum([q.v]))
c = {1, 2, 3};
disp([c{:}])
[a, b] = c{2:3};
disp(b)
y = c{:}
```

```
p =

  1×2 struct array with fields:

    name

AB
     2     1
     1     3
    60
     1     2     3
     3
Error: Line 13: Expected one output from a curly brace or dot indexing expression, but there were 3 results.
```

### Functions on cells

`cellfun` calls a function on the contents of each element and collects
scalar results in an array; with `'UniformOutput', false` it collects any
results in a cell, and so does `arrayfun`. `cellfun` also takes a function's
name. `cell2mat` joins a cell of arrays, `num2cell` splits an array into a
cell, and `deal` copies its input to every output. `for` over a cell takes
one column at a time, itself a cell:

```matlab
disp(cellfun(@numel, {'ab', 'cde', ''}))
r = cellfun(@(x) x * 2, {1, [2 3]}, 'UniformOutput', false);
disp(r{2})
disp(cellfun('isempty', {[], 1}))
r = arrayfun(@(x) x * [1 1], 1:2, 'UniformOutput', false)
disp(cell2mat({1 2; 3 4}))
x = num2cell([1 2])
[a, b] = deal(7);
fprintf('%d %d\n', a, b)
for k = {1, 'a'}
    disp(class(k))
end
```

```
     2     3     0
     4     6
   1   0
r =

  1×2 cell array

    {1×2 double}    {1×2 double}

     1     2
     3     4
x =

  1×2 cell array

    {[1]}    {[2]}

7 7
cell
cell
```

### `varargin` and `varargout`

A last parameter `varargin` collects the remaining arguments in a cell, and
a last output `varargout` supplies the remaining outputs from its elements.
`nargin` and `nargout` count all of them:

```matlab
disp(count(1, 'a', {}))
[lo, hi] = bounds([4 1 9]);
fprintf('%d %d\n', lo, hi)
function n = count(varargin)
n = nargin;
end
function varargout = bounds(v)
varargout{1} = min(v);
varargout{2} = max(v);
end
```

```
     3
1 9
```

## Control flow

### `if`

```matlab
x = 5;
if x > 10
    disp('big')
elseif x > 3
    disp('medium')
else
    disp('small')
end
if x, disp('x is true'), end
```

```
medium
x is true
```

### `for`

A `for` iterates over the **columns** of its expression, so a range gives
scalars and a matrix gives column vectors.

```matlab
for k = 1:3
    fprintf('k = %d\n', k);
end
for k = [10 20 30]
    fprintf('%d ', k);
end
fprintf('\n');
A = [1 2 3; 4 5 6];
for col = A
    disp(col')
end
```

```
k = 1
k = 2
k = 3
10 20 30 
     1     4
     2     5
     3     6
```

### `while`, `break`, `continue`

```matlab
n = 1;
while n < 100
    n = n * 3;
end
n
for k = 1:10
    if mod(k,2) == 0
        continue
    end
    if k > 7
        break
    end
    fprintf('%d ', k);
end
fprintf('\n');
```

```
n =

   243

1 3 5 7 
```

A `break` or `continue` outside a loop is an error, as in MATLAB. It used to end the script silently with
exit 0 instead of erroring. See [Differences](#differences-from-matlab).

### `switch`

The first `case` whose value matches runs, and there is no fall-through. A
`case {a, b}` list matches when any of its values does. A number matches a
number of equal value, whatever its class; a char matches a char of the same
text, and never a number by its code. `break` and `continue` inside a
`switch` act on the loop around it.

```matlab
x = 2;
switch x
    case 1
        disp('one')
    case {2, 3}
        disp('two or three')
    otherwise
        disp('other')
end
s = 'abc';
switch s
    case 'xyz'
        disp('no')
    case {'abc', 'def'}
        disp('text matches text')
end
switch 97
    case 'a'
        disp('never: a char does not match its code')
    otherwise
        disp('97 is not ''a'' here')
end
for k = 1:5
    switch k
        case 3
            break
    end
    fprintf('%d', k);
end
fprintf('\n');
```

```
two or three
text matches text
97 is not 'a' here
12
```

The value switched on must be a scalar or a character vector:

```matlab
switch [1 2]
    case 1
end
```

```
Error: Line 1: SWITCH expression must be a scalar or a character vector.
```

### `try` and `catch`

An error inside `try`, from `error` or from anything else, runs the `catch`
block instead of ending the script. `catch e` with a name on the same line
binds the error to `e`; `catch` followed by a comma or a newline binds
nothing. A `try` with no `catch` ignores the error.

```matlab
try
    x = [1 2] * [3 4];
catch
    disp('caught')
end
try
    error('MyPkg:myid', 'Value %d bad', 7)
catch e
    disp(e.identifier)
    disp(e.message)
    disp(class(e))
end
try
    undefined_thing + 1;
catch e
    disp(e.message)
end
e
```

```
caught
MyPkg:myid
Value 7 bad
MException
Unrecognized function or variable 'undefined_thing'.
e =

  MException: Unrecognized function or variable 'undefined_thing'.

```

`e` is a minimal `MException`: `e.message` and `e.identifier`, which is empty
for an error the interpreter raised itself or an `error` call without one.
Its display is SplatCrab's own one line, `  MException: <message>`, or
`  MException (<identifier>): <message>`; MATLAB lists the properties.
`rethrow(e)` raises it again unchanged, and `lasterr` is the message of the
last error, caught or not:

```matlab
try
    try
        error('inner')
    catch e
        rethrow(e)
    end
catch e2
    disp(['outer: ' e2.message])
end
disp(lasterr)
try
    error('no catch, so nothing happens')
end
disp('after')
```

```
outer: inner
inner
after
```

`e.stack` is a struct array with the fields `file`, `name` and `line`, one
element per function the error left, innermost first: the frames of
[the error trace](#the-error-trace). `file` is empty for a function local to
the script that was run, since SplatCrab is not told the script's path, and
an error raised outside every function has a 0x1 stack:

```matlab
try
    check(-1)
catch e
    e.stack
    disp(e.stack(1).name)
    disp(e.stack(1).line)
end
function check(x)
if x < 0
    error('chk:neg', 'negative')
end
end
```

```
ans =

  struct with fields:

    file: ''
    name: 'check'
    line: 10

check
    10
```

An `MException` is not an array either, so a binary operator on one is the
same refusal as on a cell, a struct or a handle:

```matlab
try
    error('x')
catch e
    e + 1
end
```

```
Error: Line 4: Operator '+' is not supported for operands of type 'MException'.
```

### What counts as true

A condition is true when it is non-empty and every element is non-zero. An
empty is false.

```matlab
if 1, disp('1 true'), end
if 0, disp('0 true'), else disp('0 false'), end
if -3, disp('-3 true'), end
if [1 1 1], disp('all ones true'), end
if [1 0 1], disp('mixed true'), else disp('mixed false'), end
if [], disp('empty true'), else disp('empty false'), end
if 'abc', disp('string true'), end
```

```
1 true
0 false
-3 true
all ones true
mixed false
empty false
string true
```

`if NaN` is an error, as in MATLAB: `NaN's cannot be converted to logicals.`

## Functions

### Local functions

A script may end in `function` blocks, which its statements, and each other,
can call. Every statement comes first: a statement after a function is
`Function definitions in a script must appear at the end of the file.`

```matlab
disp(sq(4))
r = hyp(3, 4)

function y = sq(x)
    y = x^2;
end

function h = hyp(a, b)
    h = sqrt(sq(a) + sq(b));
end
```

```
    16
r =

     5
```

```matlab
x = 1;
function f()
end
y = 2;
```

```
Error: Line 4: Function definitions in a script must appear at the end of the file.
```

The header takes every MATLAB form: `function name`, `function name(a, b)`,
`function y = name(...)` and `function [y, z] = name(...)`, with `~` for an
argument the function ignores. A function called as a statement that returns
a value sets `ans`, as a builtin does.

### Outputs, `nargin` and `nargout`

`[a, b] = f(...)` takes several outputs. Inside a function, `nargin` is how
many arguments it was called with, and `nargout` how many outputs it was asked
for: `1` in an expression, `0` as a statement. Arguments may be left off the
end, which is how a default is written; one too many is `Too many input
arguments.`

```matlab
[s, p] = sp(2, 3);
disp([s p])
disp(scale(5))
disp(scale(5, 3))
counts

function [s, p] = sp(a, b)
    s = a + b;
    p = a * b;
end

function y = scale(x, k)
    if nargin < 2
        k = 10;
    end
    y = k * x;
end

function a = counts()
    a = nargout;
    fprintf('asked for %d\n', nargout);
end
```

```
     5     6
    50
    15
asked for 0
ans =

     0
```

An output the function never assigns is an error when it is asked for:

```matlab
z = bad(1)

function y = bad(x)
end
```

```
Error: Line 1: Output argument "y" (and maybe others) not assigned during call to "bad".
```

### Scope and `return`

A function sees only its own variables: its arguments and what it assigns.
The caller's are out of reach, and what it assigns is gone when it returns.
`end` inside a function's indexing is the function's own, so `x(f(end))` is
always `x`'s `end`. `return` leaves the function at once, from inside any
loop.

```matlab
x = 1;
change_x();
disp(x)
disp(first_negative([3 1 -4 1 -5]))

function change_x()
    x = 99;
end

function k = first_negative(v)
    for k = 1:numel(v)
        if v(k) < 0
            return
        end
    end
    k = 0;
end
```

```
     1
     3
```

### Recursion

A function may call itself, 500 calls deep at most. One more is the clean
error `Maximum recursion limit of 500 reached.`, never a crash.

```matlab
disp(fact(10))
try
    forever(1)
catch e
    disp(e.message)
end

function r = fact(n)
    if n <= 1, r = 1; else, r = n * fact(n - 1); end
end

function forever(n)
    forever(n + 1);
end
```

```
     3628800
Maximum recursion limit of 500 reached.
```

### The error trace

An error that leaves a function reports the line of the script's own
statement that failed, and then one `  in <fn> (line N)` line per function
it came out of, innermost first:

```matlab
disp('start')
outer(1)

function outer(n)
    inner(n);
end

function inner(n)
    error('inner failed with %d', n);
end
```

```
start
Error: Line 2: inner failed with 1
  in inner (line 9)
  in outer (line 5)
```

An anonymous function in the trace is named by its `func2str` text, as in
`  in @(n)g(n)`; MATLAB's form there is not settled, and SplatCrab's is its
own.

### Function handles

`@name` is a handle to a named function and `@(x) body` an anonymous
function. Calling the variable that holds one calls it. An anonymous
function captures, when it is made, the value of every variable its body
reads, so changing the variable afterwards does not change the function; a
name that is not a variable then is looked up as a function when the body
runs. A handle displays under MATLAB's header:

```matlab
sq = @(x) x.^2;
disp(sq([1 2 3]))
a = 10;
f = @(x) x + a;
a = 0;
disp(f(1))
g = @abs;
disp(g(-4))
f
```

```
     1     4     9
    11
     4
f =

  function_handle with value:

    @(x)x+a

```

`@name` binds where it is made: a handle to a local function keeps calling
that function when it is passed to a file where the name means something
else, or nothing. An anonymous function runs in a workspace of its own
holding its parameters and what it captured, and counts against the
recursion limit like any call. A body that is a single call passes on the
number of outputs asked for, so `[m, i] = f(v)` works for `f = @(v) max(v)`.

`feval`, `arrayfun`, `func2str`, `str2func`, and `class` and `isa` with
`'function_handle'`:

```matlab
disp(arrayfun(@(x) x * 2, [1 2 3]))
disp(arrayfun(@(a, b) a * b, [1 2], [3 4]))
disp(feval(@(x) x + 1, 1))
f = @(v) max(v);
[m, i] = f([1 5 2]);
disp(i)
disp(func2str(@(x) [x 1] * 2))
h = str2func('@(x) x*3');
disp(h(2))
disp(isa(h, 'function_handle'))
```

```
     2     4     6
     3     8
     2
     2
@(x)[x,1]*2
     6
   1
```

`func2str` renders the function from its parse tree: no spaces around
operators and a comma between the elements of a bracket, so `[x 1]` comes
back as `[x,1]` and still means two elements. MATLAB keeps the text as
written; only the forms without brackets or spacing choices are known to
agree. `arrayfun` needs every result to be a scalar unless it is given
`'UniformOutput', false`, which returns a cell; see
[functions on cells](#functions-on-cells). A handle is
one function and not an array, so `[f g]` is refused, and so is
`[@(x) x+1]` in the source. `str2func` of an `'@(...)'` text captures
nothing, since it cannot see the workspace it is called from.

`isequal` compares handles as the MathWorks page describes: two handles to
the same named function are equal, `str2func('sin')` and `@sin` included,
and so are two handles to one local function; an anonymous function is
equal only to its copies, since two made separately may hold different
captured values, whatever their text; and a handle equals nothing but a
handle:

```matlab
fun1 = @sin;
fun2 = @sin;
disp(isequal(fun1, fun2))
disp(isequal(str2func('sin'), @sin))
A = 5;
h1 = @(x) A * x.^2;
h2 = @(x) A * x.^2;
disp(isequal(h1, h2))
h2 = h1;
disp(isequal(h1, h2))
disp(isequal(@sin, @(x) sin(x)))
```

```
   1
   1
   0
   1
   0
```

A handle is one function and not an array, and a binary operator refuses
it, `==` included, in MATLAB's sentence; call the handle first, `f(0) + 1`:

```matlab
f = @sin;
f + 1
```

```
Error: Line 2: Operator '+' is not supported for operands of type 'function_handle'.
```

### Function files, scripts and the path

A file `name.m` in the current folder, or in a folder `addpath` added, is
callable as `name`. If it starts with `function` it is a function file: its
first function is the one the file name calls, and any others are
subfunctions, private to that file. A function file's functions may all end
with `end` or all go without, in which case each one runs to the next
`function`. Any other `.m` file is a script, and calling it runs it in the
caller's workspace, so what it assigns is the caller's.

A name is looked up in this order: a variable, then a function of the running
file, then a local function of the script being run, then a file in the
current folder, then a file on the path, then a builtin. So a file on the
path shadows a builtin of its name, as in MATLAB:

    addpath('shadow')    % shadow/max.m returns 42
    max([1 5 2])         % 42
    rmpath('shadow')
    max([1 5 2])         % 5

`addpath` puts folders at the front of the path, and `rmpath` takes them off;
a relative folder is resolved against the current folder. A file is read once
and kept; it is read again when the path changes, and, from the REPL, the
protocol or the browser page, when it has changed on disk since the last
entry. Whatever the interpreter writes itself, with `fopen` and `fclose`,
`save`, `delete` or the browser editor's Save, makes every file be read
again at its next call, so a script that writes a function file and then
calls it runs what it just wrote.

`exist(name)` is `1` for a variable, `2` for a file on the path, `5` for a
builtin and `0` otherwise. `feval('name', ...)` calls a function by name.

```matlab
x = 1;
disp([exist('x') exist('max') exist('nosuch')])
disp(feval('max', [4 9 2]))
disp(feval('twice', 21))

function y = twice(x)
    y = 2 * x;
end
```

```
     1     5     0
     9
    42
```

At the REPL, in a protocol `eval` and in the browser page, a `function` block
is read to its `end` and then refused: `Function definitions are not
supported in this context.` Define functions in a script or a function file.

## Builtins

163 names, each an ordinary function registered by name. Every one rejects
arguments it does not understand with `Too many input arguments.` rather than
ignoring them. A builtin that produces no value (`disp`, `fprintf`, `clc`,
`clear`, `who`, bare `tic`, bare `toc`) is legal as a statement and is
`Too many output arguments.` in an expression.

### Constants

`pi Inf inf NaN nan eps true false`. `true` and `false` are logicals; `eps`
alone is the spacing at 1.

```matlab
a = pi
b = Inf
c = -Inf
d = NaN
e = eps
f = true
g = false
```

```
a =

    3.1416

b =

   Inf

c =

  -Inf

d =

   NaN

e =

   2.2204e-16

f =

  logical

   1

g =

  logical

   0

```

There is no `e` constant: MATLAB does not have one, so write `exp(1)`. `e` is
an ordinary name you are free to use as a variable.

### Constructors

`zeros ones eye rand linspace`. A negative size gives an empty, not an error;
a size that would overflow is a clean error naming the size you asked for.
Every constructor but `eye` and `linspace` takes three or more sizes too,
for an [N-D array](#n-d-arrays).

```matlab
Z = zeros(2,3)
O = ones(2)
I = eye(3)
L = linspace(0, 1, 5)
R = rand(2,2);
disp(size(R))
disp(all(R(:) >= 0 & R(:) < 1))
```

```
Z =

     0     0     0
     0     0     0

O =

     1     1
     1     1

I =

     1     0     0
     0     1     0
     0     0     1

L =

         0    0.2500    0.5000    0.7500    1.0000

     2     2
   1
```

`rand` is a deterministic built-in generator seeded the same way every run;
there is no `rng` to reseed it yet.

### Shape queries

`size ndims numel length isempty isscalar isvector`. `size(A)` returns the
row vector `[rows cols]`, and one more size per dimension of an [N-D
array](#n-d-arrays); `size(A, d)` returns one dimension. `length` is the
largest dimension.

```matlab
A = [1 2 3; 4 5 6];
s = size(A)
r = size(A, 1)
c = size(A, 2)
n = numel(A)
l = length(A)
e1 = isempty(A)
e2 = isempty([])
sc = isscalar(5)
vv = isvector([1 2 3])
```

```
s =

     2     3

r =

     2

c =

     3

n =

     6

l =

     3

e1 =

  logical

   0

e2 =

  logical

   1

sc =

  logical

   1

vv =

  logical

   1

```

`[r, c] = size(A)` gives the dimensions one per output; see
[Multiple return values](#multiple-return-values).

```matlab
A = [1 2 3; 4 5 6];
[r, c] = size(A)
```

```
r =

     2

c =

     3

```

The queries answer for every value, not only for arrays: a cell or a struct
array has its own size, and a function handle and an `MException` are 1x1
and never empty:

```matlab
f = @sin;
disp(size(f))
disp(isempty(f))
c = cell(2, 3);
disp(size(c))
try, error('a:b', 'm'), catch e, end
disp(numel(e))
disp(isa(e, 'MException'))
disp(isnumeric({1}))
```

```
     1     1
   0
     2     3
     1
   1
   0
```

### Rearrangement

`reshape repmat fliplr flipud flip circshift squeeze permute ipermute cat
horzcat vertcat`. `reshape` fills column-major, like MATLAB. `squeeze`,
`permute`, `ipermute` and `cat` are for [N-D arrays](#n-d-arrays), and
`repmat` tiles along every dimension it is given; `flip`, `circshift`,
`horzcat` and `vertcat` work on matrices as on N-D arrays.

```matlab
A = [1 2 3; 4 5 6];
R = reshape(A, 3, 2)
P = repmat([1 2], 2, 2)
F = fliplr([1 2 3])
U = flipud([1 2; 3 4])
```

```
R =

     1     5
     4     3
     2     6

P =

     1     2     1     2
     1     2     1     2

F =

     3     2     1

U =

     3     4
     1     2

```

These four, transpose and `sort` keep the class, so `fliplr('abc')` is
`'cba'`. `diag` and the colon do not: `diag('ab')` and `'a':'c'` are doubles,
where MATLAB keeps a char.

### Reductions

`sum prod mean any all max min cumsum cumprod`. They work down columns by
default, and take an optional dimension.

```matlab
A = [1 2; 3 4];
a = sum(A)
b = sum(A, 2)
c = prod([1 2 3 4])
d = mean(A)
e = any([0 0 1])
f = all([1 1 0])
g = max([3 1 4 1 5])
h = min(A)
i = cumsum([1 2 3 4])
j = cumprod([1 2 3 4])
```

```
a =

     4     6

b =

     3
     7

c =

    24

d =

     2     3

e =

  logical

   1

f =

  logical

   0

g =

     5

h =

     1     2

i =

     1     3     6    10

j =

     1     2     6    24

```

A dimension past the array's hands the input back with its values as they
were, a `-0` included, as a double (`sum('ab', 3)` is `97 98`), or as a
logical for `max` and `min` of a logical. `sum` and `mean` do the same along
a dimension of size 1, as the MathWorks pages say ("or when size(A,dim) is
1"), so `sum(-0)` is `-0`; every other reduction reduces each element on its
own there. `0` is an error. A char is never read as a dimension. `any`
ignores `NaN`, so `any(NaN)` is `0` and `all(NaN)` is `1`, matching the
MATLAB page, and past the array's dimensions each element gets that same
answer, a logical of the array's shape, so `any([0 NaN 2], 3)` is `0 0 1`.

```matlab
A = [1 2 3; 4 5 6];
a = sum(A, 1)
b = sum(A, 2)
c = sum(A, 3)
d = cumsum(A, 2)
e = max(A, [], 2)
```

```
a =

     5     7     9

b =

     6
    15

c =

     1     2     3
     4     5     6

d =

     1     3     6
     4     9    15

e =

     3
     6

```

`max` and `min` also compare two arrays elementwise:

```matlab
A = [1 2; 3 4];
a = max(A)
b = max(A, [], 2)
c = max([1 5 3], [4 2 6])
d = min(A, 2)
```

```
a =

     3     4

b =

     2
     4

c =

     4     5     6

d =

     1     2
     2     2

```

### Elementwise math

`abs sqrt exp log log2 log10 sin cos tan asin acos atan sinh cosh tanh floor
ceil round fix sign`. All map over every element.

```matlab
a = abs([-1 2 -3])
b = sqrt([4 9 16])
c = exp(1)
d = log(exp(2))
e = log2(8)
f = log10(1000)
g = sin(pi/2)
h = cos(0)
i = tan(pi/4)
j = round([1.4 1.5 -1.5])
k = floor([1.7 -1.7])
l = ceil([1.2 -1.2])
m = fix([1.7 -1.7])
n = sign([-5 0 5])
```

```
a =

     1     2     3

b =

     2     3     4

c =

    2.7183

d =

     2

e =

     3

f =

     3

g =

     1

h =

     1

i =

    1.0000

j =

     1     2    -2

k =

     1    -2

l =

     2    -1

m =

     1    -1

n =

    -1     0     1

```

`round` breaks ties away from zero, as MATLAB does. `sqrt`, `exp`, `log`,
`log2`, `log10`, `sin`, `cos`, `asin`, `acos` and `abs` take complex values,
and a real argument whose answer is complex — `sqrt(-4)`, `log(-1)`,
`asin(2)` — gives the complex value on the principal branch; see
[Complex numbers](#complex-numbers). The rest of this list refuses a complex
argument.

### Predicates

`isnan isinf isfinite`, beside the shape queries `isempty isscalar isvector`
and the class tests `islogical ischar isnumeric isa`. Every one returns a
logical. `isequal(A, B, ...)` is true when every argument has the first
one's size and values; the class is not compared, so `isequal('a', 97)` is
true. Cells and structs compare element by element and function handles by
what they name; see [cell arrays](#cell-arrays), [structs](#structs) and
[function handles](#function-handles).

```matlab
a = isnan([1 NaN Inf])
b = isinf([1 NaN Inf])
c = isfinite([1 NaN Inf])
```

```
a =

  1×3 logical array

   0   1   0

b =

  1×3 logical array

   0   0   1

c =

  1×3 logical array

   1   0   0

```

```matlab
a = isequal([1 2], [1 2])
b = isequal('a', 97)
c = isequal([1 2], [1 2 3])
```

```
a =

  logical

   1

b =

  logical

   1

c =

  logical

   0

```

### Two-argument math

`mod rem atan2 hypot power`. `mod` takes the sign of the divisor, `rem` the
sign of the dividend.

```matlab
a = mod(-7, 3)
b = rem(-7, 3)
c = atan2(1, 1)
d = hypot(3, 4)
e = power(2, 10)
```

```
a =

     2

b =

    -1

c =

    0.7854

d =

     5

e =

        1024

```

### Linear algebra

`transpose inv det trace diag norm dot`, and since cycle 08 the
factorisations `lu qr chol eig svd`, the builtins over them
`rank pinv null orth cond`, and `kron cross triu tril magic`. `det`, `inv`,
`\`, `/`, `A^-n` and `lu` share one LU factorisation with partial pivoting,
which is what makes `det` and `\` agree on what singular means.

```matlab
A = [4 -2; 1 1];
a = transpose(A)
b = inv(A)
c = det(A)
d = trace(A)
e = diag(A)
f = diag([1 2 3])
g = norm([3 4])
h = dot([1 2 3], [4 5 6])
```

```
a =

     4     1
    -2     1

b =

    0.1667    0.3333
   -0.1667    0.6667

c =

     6

d =

     5

e =

     4
     1

f =

     1     0     0
     0     2     0
     0     0     3

g =

     5

h =

    32

```

A non-square system has a least-squares solution: here the line through
`(1, 1)`, `(2, 2)` and `(3, 2)`, intercept first.

```matlab
A = [1 1; 1 2; 1 3];
b = [1; 2; 2];
x = A \ b
y = b' / A'
```

```
x =

    0.6667
    0.5000

y =

    0.6667    0.5000

```

`norm` of a matrix takes `1`, `2`, `Inf` and `'fro'`; `cond` is the 2-norm
condition number. The factorisations return as many outputs as asked for:

```matlab
A = [1 2; 3 4];
n = [norm(A, 1), norm(A, Inf)]
fprintf('%.4f %.4f %.4f\n', norm(A), norm(A, 'fro'), cond(A));
[L, U, P] = lu(A)
R = chol([4 2; 2 3])
fprintf('%.4f %.4f\n', eig([2 1; 1 2]), svd([3 0; 0 4]));
r = rank([1 2; 2 4])
```

```
n =

     6     7

5.4650 5.4772 14.9330
L =

    1.0000         0
    0.3333    1.0000

U =

    3.0000    4.0000
         0    0.6667

P =

     0     1
     1     0

R =

    2.0000    1.0000
         0    1.4142

1.0000 3.0000
4.0000 3.0000
r =

     1

```

`eig` and `svd` are iterations, so their values can be a roundoff away from
a whole number even when the exact answer is one: print them with `fprintf`,
as here, or compare them with a tolerance, never with `==`. `eig` of a
symmetric matrix is in ascending order and `svd` in descending order. The
signs of `Q`, `R`, `U`, `V` and the eigenvectors are conventions, not unique,
and so are the bases `null` and `orth` return: check a factorisation by its
residual and a basis by its properties.

```matlab
[Q, R] = qr([1 2; 3 4]);
disp(norm(Q * R - [1 2; 3 4]) < 1e-12)
A = [2 1; 1 2];
[V, D] = eig(A);
disp(norm(A * V - V * D) < 1e-12)
N = null([1 1]);
O = orth([1 2; 2 4]);
disp([size(N), size(O)])
p = pinv([1 2; 2 4])
k = kron([1 2], [1; 1])
c = cross([1 0 0], [0 1 0])
m = magic(4)
t = triu(magic(3))
l = tril(magic(3), -1)
```

```
   1
   1
     2     1     2     1
p =

    0.0400    0.0800
    0.0800    0.1600

k =

     1     2
     1     2

c =

     0     0     1

m =

    16     2     3    13
     5    11    10     8
     9     7     6    12
     4    14    15     1

t =

     8     1     6
     0     5     7
     0     0     2

l =

     0     0     0
     3     0     0
     4     9     0

```

A singular matrix is a warning, not an error, as in MATLAB: `inv` returns
`Inf` everywhere, and `\` returns whatever the substitution gives. The
warnings go to stderr, which is why they come after the output here:

```matlab
A = [1 2; 2 4];
b = inv(A)
x = A \ [1; 2];
```

```
b =

   Inf   Inf
   Inf   Inf

Warning: Matrix is singular to working precision.
Warning: Matrix is singular to working precision.
```

A real matrix whose eigenvalues include a complex pair gives the pair as
complex values, the one with the positive imaginary part first, and
`[V, D] = eig(A)` gives complex eigenvectors for them. `eig` of a complex
matrix, and a `NaN` or `Inf` handed to `eig` or `svd`, are refused:

```matlab
A = [0 -1; 1 0];
e = eig(A)
[V, D] = eig(A);
disp(norm(abs(A * V - V * D)) < 1e-12)
```

```
e =

   0.0000 + 1.0000i
   0.0000 - 1.0000i

   1
```

Integer matrix powers work, negative ones by inverting:

```matlab
A = [4 -2; 1 1];
a = A^-1
b = A^0
fprintf('%.4f\n', sum([]));
fprintf('%.4f\n', norm([]));
```

```
a =

    0.1667    0.3333
   -0.1667    0.6667

b =

     1     0
     0     1

0.0000
0.0000
```

### Search and sort

`find sort sub2ind ind2sub`. Since cycle 09 `sort` takes a matrix too, and
since cycle 14c any array along any dimension ([N-D arrays](#n-d-arrays)),
where `sub2ind` and `ind2sub` are.

```matlab
x = [0 3 0 7 5];
a = find(x)
b = sort(x)
v = [3 1 2];
c = sort(v)
```

```
a =

     2     4     5

b =

     0     0     3     5     7

c =

     1     2     3

```

A matrix is sorted column by column, each column on its own, or row by row
with `sort(A, 2)`. The second output is each column's (or row's)
permutation, so `s(:, j)` is `A(i(:, j), j)`. `NaN` placement, stability and
the class rules are the vector's.

```matlab
A = [3 1; 2 4];
a = sort(A)
b = sort(A, 2, 'descend')
[s, i] = sort(A);
i
```

```
a =

     2     1
     3     4

b =

     3     1
     4     2

i =

     2     1
     1     2

```

### Polynomials

`polyfit polyval roots conv deconv` (cycle 09). A polynomial is a row of
coefficients, highest power first. `polyfit` solves its Vandermonde system
by the column-pivoted QR of `\`; `roots` takes the eigenvalues of the
companion matrix, as MATLAB's does, so their order is the eigensolver's and
a script that needs an order sorts them. `[q, r] = deconv(b, a)` divides,
with `b = conv(a, q) + r`.

```matlab
p = polyfit([0 1 2 3], [1 3 5 7], 1);
fprintf('%.4f ', p); fprintf('\n');
r = sort(roots([1 -6 11 -6]));
fprintf('%.4f ', r); fprintf('\n');
a = polyval([1 0 -1], [2 3])
c = conv([1 2], [1 3])
[q, rem] = deconv([1 5 7], [1 2])
```

```
2.0000 1.0000
1.0000 2.0000 3.0000
a =

     3     8

c =

     1     5     6

q =

     1     3

rem =

     0     0     1

```

Complex roots come as complex values, a conjugate pair with the positive
imaginary part first. A real root of multiplicity three or more comes back complex too, a pair a
rounding error apart, as the eigensolver finds it and as MATLAB's does:

```matlab
roots([1 0 1])
```

```
ans =

   0.0000 + 1.0000i
   0.0000 - 1.0000i

```

### Samples

`interp1 trapz cumtrapz diff filter` (cycle 09). `interp1(x, v, xq)`
interpolates linearly by default, or with `'nearest'`, `'previous'` or
`'next'`; a query outside the sample points is `NaN` unless `'extrap'` or a
value follows the method. `trapz` and `cumtrapz` integrate by the
trapezoidal rule, with unit spacing or the sample points given first.
`diff(X, n)` differences `n` times. `filter(b, a, x)` applies the
difference equation with `a(1)` normalising. Each works down the columns of
a matrix, and all but `interp1` and `filter` take a dimension.

```matlab
a = interp1([1 2 3], [10 20 30], [1.5 2.5 4])
b = interp1([1 2 3], [10 20 30], 2.4, 'nearest')
c = interp1([1 2 3], [10 20 30], 4, 'linear', 'extrap')
d = trapz([0 1 2], [0 1 4])
e = cumtrapz([1 2 3])
f = diff([1 4 9 16], 2)
g = filter(1, [1 -0.5], [1 0 0 0])
```

```
a =

    15    25   NaN

b =

    20

c =

    40

d =

     3

e =

         0    1.5000    4.0000

f =

     2     2

g =

    1.0000    0.5000    0.2500    0.1250

```

### Statistics and number theory

`std var median mode factorial nchoosek primes isprime gcd lcm` (cycle 09).
`std` and `var` normalise by `N - 1`, or by `N` with a weight of `1`
second. `median` of anything holding a `NaN` is `NaN`; `mode` ignores `NaN`
and gives the smallest of a tie. All four work down the columns of a
matrix and take a dimension, any dimension of any array since cycle 14c. `nchoosek(v, k)` of a vector lists the
combinations, one per row.

```matlab
x = [2 4 4 4 5 5 7 9];
fprintf('%.4f %.4f\n', std(x), var(x, 1));
a = median(x)
b = mode(x)
c = median([1 3; 2 8])
d = factorial(5)
e = nchoosek(5, 2)
f = nchoosek([1 2 3], 2)
g = primes(20)
h = isprime([2 9 11])
k = [gcd(12, 18), lcm(4, 6)]
```

```
2.1381 4.0000
a =

    4.5000

b =

     4

c =

    1.5000    5.5000

d =

   120

e =

    10

f =

     1     2
     1     3
     2     3

g =

     2     3     5     7    11    13    17    19

h =

  1×3 logical array

   1   0   1

k =

     6    12

```

### Sets

`unique ismember setdiff intersect union` (cycle 09), over arrays and over
cells of character vectors, which sort by character code as `sort` sorts
chars. A result is sorted, or in the order of first occurrence with
`'stable'`; it is a row when the inputs are rows. `[C, ia, ic] = unique(A)`
gives each value's first position and every element's place in `C`, and
`[tf, loc] = ismember(A, S)` the lowest position in `S`. A character
vector beside a cell is one word.

```matlab
[u, ia] = unique([3 1 2 1])
[tf, loc] = ismember([2 5], [1 2 3])
names = unique({'bob', 'al', 'bob'})
d = setdiff({'a', 'b', 'c'}, {'b'})
i = intersect([5 1 3], [3 4 5])
w = union([3 1], [2 1])
```

```
u =

     1     2     3

ia =

     2
     3
     1

tf =

  1×2 logical array

   1   0

loc =

     2     0

names =

  1×2 cell array

    {'al'}    {'bob'}

d =

  1×2 cell array

    {'a'}    {'c'}

i =

     3     5

w =

     1     2     3

```

### Grids and counts

`logspace meshgrid histc` (cycle 09). `histc(x, edges)` counts
`edges(k) <= x < edges(k+1)`, the last bin `x == edges(end)`.

```matlab
v = logspace(0, 2, 3)
[X, Y] = meshgrid(1:3, 1:2)
n = histc([1 2 2 3 5], [1 2 3 4])
```

```
v =

     1    10   100

X =

     1     2     3
     1     2     3

Y =

     1     1     1
     2     2     2

n =

     1     2     1     0

```

### Solvers

`fzero fminsearch integral ode45 odeset` (cycle 09). Each takes a function
handle (or a function's name) and calls it through the interpreter, so it
may be anonymous, local or on the path, and may call a solver itself.
`fzero(f, x0)` searches outward from `x0` for a sign change and then runs
Brent's method; `fzero(f, [a b])` starts from the bracket. `fminsearch` is
Nelder-Mead. `integral` is adaptive Gauss-Kronrod quadrature with
MATLAB's tolerances, and takes infinite limits; its function is called
with a row of points, so write it with `.*`, `./` and `.^`. `ode45` is the
Dormand-Prince pair with `odeset`'s `RelTol`, `AbsTol`, `MaxStep`,
`InitialStep` and `Refine`, and gives `[t, y]` with one row of `y` per time.

A solver stops at a tolerance, so its last digits are roundoff: print a
few places, or compare against the tolerance, as these do.

```matlab
fprintf('%.6f\n', fzero(@(x) x^2 - 2, 1));
fprintf('%.6f\n', fzero(@(x) cos(x) - x, [0 1]));
x = fminsearch(@(x) (x(1) - 1)^2 + (x(2) - 2)^2, [0 0]);
disp(norm(x - [1 2]) < 1e-3)
fprintf('%.4f\n', integral(@(x) x.^2, 0, 1));
fprintf('%.4f\n', integral(@(x) exp(-x.^2), -Inf, Inf));
opts = odeset('RelTol', 1e-6);
[t, y] = ode45(@(t, y) [y(2); -y(1)], [0 pi], [0; 1], opts);
disp(abs(y(end, 1)) < 1e-4)
```

```
1.414214
0.739085
   1
0.3333
1.7725
   1
```

Every solver has a cap, and meeting it is a clean error rather than a hang
or a `NaN` handed back as an answer: `fzero` with no sign change to find,
`fminsearch` past `200 * numel(x0)` iterations or evaluations, `integral`
past 650 subintervals, and `ode45` past 50,000 steps or below its smallest
step. MATLAB warns and returns a value in some of these cases.

```matlab
integral(@(x) 1 ./ x, 0, 1)
```

```
Error: Line 1: 'integral' reached its limit of 650 subintervals without meeting the tolerance; the integral may not exist.
```

### Multiple return values

`[a, b] = f(...)` asks a builtin for two values. `max`, `min`, `sort`, `size`
and `find` give more than one: `max` and `min` the index of the extreme value
too (the first of a tie), `sort` the permutation, `size` one dimension per
output (then `1`s), and `find` the rows and columns. A `~` discards the output
in its place. Each output is displayed in turn unless the statement ends in
`;`, and `ans` is not set.

```matlab
[m, i] = max([3 9 2])
[r, c] = size(zeros(2, 5))
[~, k] = min([4 2 8])
[s, idx] = sort([3 1 2])
```

```
m =

     9

i =

     2

r =

     2

c =

     5

k =

     2

s =

     1     2     3

idx =

     2     3     1

```

With three outputs `find` also returns the values, in the argument's class:

```matlab
[r, c, v] = find([0 7; 5 0])
```

```
r =

     2
     1

c =

     1
     2

v =

     5
     7

```

Asking a builtin for more values than it has is an error, and so is asking a
plain value for more than one:

```matlab
[a, b] = sum([1 2])
```

```
Error: Line 1: Too many output arguments.
```

```matlab
[a, b] = 5
```

```
Error: Line 1: Insufficient number of outputs from right hand side of equal sign to satisfy assignment.
```

### Output

`disp fprintf sprintf num2str`, and `error rethrow lasterr warning assert`,
which [Errors and exit codes](#errors-and-exit-codes) and
[`try` and `catch`](#try-and-catch) describe. See
[Output and formatting](#output-and-formatting) for the detail.

```matlab
disp('plain text')
disp([1 2 3])
fprintf('%d apples\n', 5);
s = sprintf('%5.2f', pi)
n = num2str(pi)
```

```
plain text
     1     2     3
5 apples
s =

    ' 3.14'

n =

    '3.1416'

```

### Strings and text

`strcat strsplit strjoin strrep strtrim upper lower strfind strtok blanks`
(cycle 11). `strcat` drops the trailing whitespace of a char argument,
where `[...]` keeps it; `strsplit` collapses a run of delimiters into one
unless `'CollapseDelimiters'` is false; `strrep` and `strfind` count
overlapping occurrences.

```matlab
s = strcat('file', '_', 'name')
c = strsplit('a,b,,c', ',')
j = strjoin({'x', 'y', 'z'}, ' + ')
r = strrep('one two two', 'two', '2')
t = strtrim(sprintf('  padded\t'))
u = upper('Mixed Case')
k = strfind('abcabc', 'bc')
[tok, rest] = strtok('first second third')
```

```
s =

    'file_name'

c =

  1×3 cell array

    {'a'}    {'b'}    {'c'}

j =

    'x + y + z'

r =

    'one 2 2'

t =

    'padded'

u =

    'MIXED CASE'

k =

     2     5

tok =

    'first'

rest =

    ' second third'
```

The comparisons `strcmp strcmpi strncmp strncmpi` and the predicates
`isspace isletter` return logicals. A cell of text compares element by
element:

```matlab
a = strcmp('abc', 'abc')
b = strcmpi('ABC', 'abc')
c = strcmp({'red', 'green', 'red'}, 'red')
d = strncmp('prefix_one', 'prefix_two', 7)
e = isspace('a b')
```

```
a =

  logical

   1

b =

  logical

   1

c =

  1×3 logical array

   1   0   1

d =

  logical

   1

e =

  1×3 logical array

   0   1   0
```

Numbers and text convert both ways. `num2str` of a matrix gives one row
per matrix row, as MATLAB lays it out; `mat2str` writes MATLAB syntax;
`str2double` gives `NaN` for text that is not a number; `str2num` reads
literals and operators only, never a function call or a variable.

```matlab
a = num2str([1 2; 3 4])
b = num2str([1.5 -2 10])
c = int2str(2.5)
d = mat2str([1 2; 3 4.5])
e = str2double('1.5e3')
f = str2double('twelve')
g = str2num('[1 2 3] * 2')
```

```
a =

  2×4 char array

    '1  2'
    '3  4'

b =

    '1.5            -2            10'

c =

    '3'

d =

    '[1 2;3 4.5]'

e =

        1500

f =

   NaN

g =

     2     4     6
```

`num2str(A, n)` gives each element `n` significant digits and
`num2str(A, format)` formats each row with the format, one char row per
matrix row either way. `upper`, `lower`, `strtrim`, `strrep`, `strcat`,
`strjoin`, `strfind`, `str2double` and the `strcmp` family take a cell of
text too, element by element:

```matlab
a = num2str([pi; -2], 4)
b = num2str([1 2; 3 4], '%d,')
c = upper({'ab', 'cd'})
d = strrep({'aa', 'ba'}, 'a', 'x')
```

```
a =

  2×5 char array

    '3.142'
    '   -2'

b =

  2×4 char array

    '1,2,'
    '3,4,'

c =

  1×2 cell array

    {'AB'}    {'CD'}

d =

  1×2 cell array

    {'xx'}    {'bx'}
```

Where a function takes a character vector, a char of at most one row, a
char of several rows is refused, as an argument or as an element of a
cell, in the words that place refuses any value that is not text, as the
MathWorks `iscellstr` page says most text functions do: `upper({['ab';
'cd']})` is `Every element of a cell argument to 'upper' must be a
character vector.` A bare char array of any shape still goes to `upper`,
`lower` and `strcat`, which keep its rows, and `strtrim` trims an element
of several rows as it trims that array on its own:

```matlab
x = upper(['ab'; 'cd'])
r = strtrim({[' ab'; ' cd']});
disp(r{1})
strrep(['ab'; 'cd'], 'a', 'z')
```

```
x =

  2×2 char array

    'AB'
    'CD'

ab
cd
Error: Line 4: Argument 1 to 'strrep' must be a character vector.
```

### Regular expressions

`regexp` and `regexprep` run on SplatCrab's own engine, which takes time
linear in its input whatever the pattern, so a pattern that makes a
backtracking engine hang, `(a*)*b` on a long run of `a`s, returns at once.
The outputs are asked for by name, `'match'`, `'tokens'`, `'names'`,
`'start'`, `'end'`, `'split'` and `'tokenExtents'`, in the order
given; `'once'` gives the first match alone. `$N` in a replacement is
token `N`.

```matlab
m = regexp('x=12, y=345', '\d+', 'match')
[tok, pos] = regexp('x=12, y=345', '(\w)=(\d+)', 'tokens', 'start');
disp(tok{2}{1})
disp(pos)
n = regexp('key=value', '(?<k>\w+)=(?<v>\w+)', 'names')
s = regexprep('2024-01-31', '(\d+)-(\d+)-(\d+)', '$3/$2/$1')
f = regexp('one two', '\w+', 'match', 'once')
```

```
m =

  1×2 cell array

    {'12'}    {'345'}

y
     1     7
n =

  struct with fields:

    k: 'key'
    v: 'value'

s =

    '31/01/2024'

f =

    'one'
```

The syntax the engine takes:

| Syntax | Meaning |
|---|---|
| `abc`, `\.`, `\(`, `\\` | literals; `\` before any other character is that character |
| `.` | any character, the newline included |
| `[abc]`, `[a-z]`, `[^...]` | classes; `\d \w \s` and the escapes work inside one |
| `\d \w \s`, `\D \W \S` | digit, word character (`_` included), whitespace, and their negations |
| `\n \t \r \f \v \a \e \0`, `\b` | escapes; `\b` is the backspace, as in MATLAB, not a word boundary |
| `\xN`, `\x{N}`, `\oN`, `\o{N}` | a character by its hexadecimal or octal code, up to U+FFFF |
| `^`, `$`, `\<`, `\>` | the start and end of the text, and of a word |
| `a\|b`, `(...)`, `(?:...)`, `(?<name>...)`, `(?#...)` | alternation, groups, non-capturing and named groups, comments |
| `*`, `+`, `?`, `{n}`, `{n,}`, `{n,m}` | quantifiers, each lazy with a `?` after it; a count is at most 1000 |

`'ignorecase'` folds case, `'emptymatch'` keeps empty matches, and
`regexprep`'s replacement takes `$0`, `$N` and `$<name>`. A cell of text
gives a cell of answers, one per element:

```matlab
p = regexp('a1,b22;c333', '[,;]', 'split')
[s, e] = regexp('say HELLO hello', 'hello', 'ignorecase')
w = regexprep('one two', '\<(\w)', '<$1>')
t = regexp('ab12', '(?<letters>[a-z]+)(?<digits>\d+)', 'names');
disp(t.digits)
try
    regexp('aa', '(a)\1')
catch err
    disp(err.message)
end
```

```
p =

  1×3 cell array

    {'a1'}    {'b22'}    {'c333'}

s =

     5    11

e =

     9    15

w =

    '<o>ne <t>wo'

12
Backreferences are not supported in regular expressions.
```

A backreference, `(a)\1` or `\k<name>`, cannot be matched in linear time
and is an error, as are lookahead and lookbehind, atomic groups,
possessive quantifiers, conditionals and inline flags such as `(?i)`; use
the `'ignorecase'` option for the last. A pattern nested more than 250
deep, or one whose compiled form is past 20,000 instructions, is `The
regular expression is too large.`

### Files

`fopen fclose fgetl fgets fprintf fread fwrite feof fileread readmatrix
writematrix csvread csvwrite delete save load input`. A relative name is
found from the current folder. `fopen` returns `-1` for a file it cannot
open; `fgetl` returns `-1` at the end of the file, which is not a char.
`fprintf(1, ...)` writes to the output and `fprintf(2, ...)` to stderr,
and `n = fprintf(...)` is the number of bytes written.

```matlab
fid = fopen('notes.txt', 'w');
n = fprintf(fid, 'line %d\n', 1:3);
fclose(fid);
disp(n)
fid = fopen('notes.txt');
while true
    l = fgetl(fid);
    if ~ischar(l), break, end
    disp(upper(l))
end
fclose(fid);
disp(fileread('notes.txt'))
delete('notes.txt')
```

```
    21
LINE 1
LINE 2
LINE 3
line 1
line 2
line 3
```

`writematrix` and `readmatrix` write and read comma-separated numbers, and
`save` and `load` a MAT-file of version 5, uncompressed, which MATLAB
reads. `load` with an output gives a struct:

```matlab
A = [1 2.5; 3 4];
writematrix(A, 'a.csv');
disp(fileread('a.csv'))
B = readmatrix('a.csv')
x = magic(3);
label = 'three';
save('work.mat', 'x', 'label');
clear
load('work.mat')
disp(label)
disp(x(2, :))
S = load('work.mat', 'x');
disp(fieldnames(S))
delete('a.csv', 'work.mat')
```

```
1,2.5
3,4

B =

    1.0000    2.5000
    3.0000    4.0000

three
     3     5     7
    {'x'}
```

`fwrite` and `fread` move binary data, `uint8` unless a precision such as
`'uint16'`, `'int32'` or `'double'` is named, little-endian; `fread` gives
a column and the count. `feof` is `1` once a read has reached the end of
the file. `csvwrite` writes five significant digits and `csvread` skips a
number of rows and columns; `save -ascii` writes each value `%.7e` and
`load` of a text file names the variable after the file:

```matlab
fid = fopen('bytes.bin', 'w');
count = fwrite(fid, [1 2 300], 'uint16')
fclose(fid);
fid = fopen('bytes.bin');
[v, n] = fread(fid, Inf, 'uint16');
disp(v')
disp(feof(fid))
fclose(fid);
csvwrite('c.csv', [1/3 2; 3 4]);
disp(fileread('c.csv'))
m = csvread('c.csv', 1, 0)
x = [1 -2.5];
save('x.txt', 'x', '-ascii');
disp(fileread('x.txt'))
load('x.txt');
disp(x)
delete('bytes.bin', 'c.csv', 'x.txt')
```

```
count =

     3

     1     2   300
     1
0.33333,2
3,4

m =

     3     4

   1.0000000e+00  -2.5000000e+00

    1.0000   -2.5000
```

`delete` takes one or more files, each by its full name: a wildcard,
`delete('*.txt')`, is an error rather than expanded, and a file that is
not there is a warning.
`load` checks every size a MAT-file claims before it allocates anything,
so a truncated or corrupt file is an error. A compressed MAT-file,
MATLAB's default since version 7, is not read: save it there with `-v6`.
`save` in an empty workspace is an error, since the file would hold no
variable. An error about a file names it and gives the reason in words
that are the same on every platform, `No such file or directory` or `It is
a directory`.

`input(prompt)` writes its prompt, reads a line and evaluates it, and
`input(prompt, 's')` returns the line as text. Under `--protocol`, `--ui`
and `--http-stdio` there is no terminal and it is an error, `input is not
available in this session: there is no terminal to read from.`, with the
session going on; at the end of standard input it is `'input' reached the
end of standard input.`

### Plotting

`figure gcf close clf subplot plot scatter bar histogram xlabel ylabel
title legend grid axis xlim ylim hold saveas print`. The builtins change a
figure's state and print nothing; a figure is drawn when it is saved.
`saveas(gcf, 'f.svg')` writes SVG, and `saveas(gcf, 'f.png')` or
`print('-dpng', 'f.png')` a PNG of 560x420 pixels. Every path is taken
against the current folder, like `fopen`'s.

**Plot kinds.** `plot(y)`, `plot(x, y)` and `plot(x1, y1, spec1, x2, y2,
...)` draw lines, one for each column of a matrix. A line spec such as
`'r--o'` sets a colour (`rgbcmykw`), a line style (`-`, `--`, `:`, `-.`)
and a marker (`o+*.xsd^v<>ph`); a series without a colour takes MATLAB's
colour order in turn. `scatter(x, y, sz, c, 'filled')` draws a circle at
each point, `bar(y)` and `bar(x, y, width)` a bar for each value, grouped
for a matrix, and `histogram(x)`, `histogram(x, nbins)` and
`histogram(x, edges)` bars of counts.

In the SVG each axes is a `<g class="axes">` group, each line a
`<polyline>`, each scatter point and `o` marker a `<circle>`, each bar a
`<rect class="bar">` and each label a `<text>` holding the label alone,
escaped, so a script can check what it drew:

```matlab
x = 1:5;
plot(x, x.^2, 'o-');
hold on
plot(x, 2*x, 'k:');
legend('square', 'double');
title('Two lines');
v = axis
saveas(gcf, 'lines.svg');
s = fileread('lines.svg');
delete('lines.svg')
disp(numel(strfind(s, '<polyline')))
disp(numel(strfind(s, '<circle')))
disp(~isempty(strfind(s, '>square<')))
```

```
v =

     1     5     0    25

     2
     5
   1
```

**Labels, legends and axes.** `xlabel`, `ylabel` and `title` take one
char vector; labels are plain text. `legend('a', 'b')` or
`legend({'a', 'b'})` names the series in order, `legend` alone names them
`data1`, `data2`, ..., and `legend off` removes it. `grid on`, `grid off`
and `grid` show, hide and toggle grid lines at the ticks. `xlim([lo hi])`,
`ylim([lo hi])` and `axis([x0 x1 y0 y1])` fix the limits, an infinite end
staying automatic, and with no argument they return the limits shown;
`axis` also takes `tight`, `equal`, `square`, `auto`, `off` and more.
Automatic limits widen to the tick step, a multiple of 1, 2 or 5 times a
power of ten:

```matlab
scatter([1 2 3 4], [2 4 1 3], 'r', 'filled');
xlabel('time'); ylabel('level');
grid on
xlim([0 5]);
v = axis
saveas(gcf, 'points.svg');
s = fileread('points.svg');
delete('points.svg')
fprintf('%d circles\n', numel(strfind(s, '<circle')))
fprintf('%d %d\n', ~isempty(strfind(s, '>time<')), ~isempty(strfind(s, 'class="grid"')))
```

```
v =

     0     5     1     4

4 circles
1 1
```

**Hold.** With hold off, the default, each `plot`, `scatter`, `bar` or
`histogram` replaces what the axes held, its labels, legend and limits
included. `hold on` keeps it, and the colour order carries on:

```matlab
plot(1:3);
hold on
plot(3:-1:1);
hold off
saveas(gcf, 'held.svg');
a = numel(strfind(fileread('held.svg'), '<polyline'));
plot(1:2);
saveas(gcf, 'held.svg');
b = numel(strfind(fileread('held.svg'), '<polyline'));
delete('held.svg')
fprintf('%d lines, then %d\n', a, b)
```

```
2 lines, then 1
```

**Subplots.** `subplot(m, n, p)` makes the `p`-th axes of an `m`-by-`n`
grid current, counting along the rows from the top left; `subplot(211)` is
the same as `subplot(2, 1, 1)`. Each axes keeps its own series, labels and
hold, and `axis` answers for the current one:

```matlab
figure;
subplot(2, 1, 1); bar([3 1 2]);
subplot(2, 1, 2); histogram([1 2 2 3 3 3], 3);
v = axis
saveas(gcf, 'grid.svg');
s = fileread('grid.svg');
delete('grid.svg')
fprintf('%d axes, %d bars\n', numel(strfind(s, 'class="axes"')), numel(strfind(s, '<rect class="bar"')))
```

```
v =

     1     3     0     3

2 axes, 6 bars
```

**Figures.** `gcf` is the current figure's number, a figure being made if
none is open; `figure` makes a new one with the lowest free number,
`figure(n)` makes figure `n` current, `close` closes the current one (the
one current before it becomes current again), `close(n)` figure `n`,
`close all` every one, and `clf` empties the current one:

```matlab
disp(gcf)
figure(5);
figure;
disp(gcf)
close
disp(gcf)
close all
figure;
disp(gcf)
```

```
     1
     2
     5
     1
```

**Saving.** `saveas(fig, name)` takes the format from the extension, and
a name with none is an error that says so; `saveas(fig, name, fmt)` takes
it from `fmt`. `print` saves the current figure:
`-dpng` or `-dsvg` names the format, `-r<dpi>` a PNG's resolution in dots
an inch (96 by default, and for `-r0`), `-f<n>` another figure, and a name
with no extension gets the format's. `print` with no file name is an
error, since there is no printer:

```matlab
plot(1:3);
print('-dpng', '-r192', 'line.png');
fid = fopen('line.png'); b = fread(fid, 24); fclose(fid);
delete('line.png')
disp(b(1:8)')
disp(b(17:24)')
print('-dsvg', 'line');
disp(numel(strfind(fileread('line.svg'), '<polyline')))
delete('line.svg')
```

```
   137    80    78    71    13    10    26    10
     0     0     4    96     0     0     3    72
     1
```

The first row is the PNG signature and the second the image's width and
height, 1120 and 840 at 192 dots an inch.

**Limits.** A figure holds at most 16,777,216 points. A line's vertex
counts one, and everything else what its SVG takes in vertices' worth: a
scatter circle 6, a bar 9, a marker from 6 for `o` to 17 for a
hexagram, so a million hexagrams are refused. The count is judged from
the arguments before a plotting call copies or changes anything, and a
PNG's pixel size is judged like any array's, so a huge resolution is
refused before a file is written. A bad argument is a clean error:

```matlab
plot(1:3);
try, print('-dpng', '-r100000', 'big.png'); catch e, disp(e.message), end
disp(fopen('big.png'))
try, close(7); catch e, disp(e.message), end
try, plot(1:3, 'LineWidth', 2); catch e, disp(e.message), end
saveas(gcf, 'f.jpg')
```

```
Requested 437500x583333 array exceeds the maximum array size.
    -1
Invalid figure handle.
Invalid line specification 'LineWidth'.
Error: Line 6: Unsupported format 'jpg' for 'saveas'; SplatCrab writes svg and png.
```

**The viewer.** At the REPL, when standard input is a terminal, every
figure an entry changed opens in the system's viewer, as a temporary SVG
file. A script never opens one, and neither do `--protocol`, `--ui` and
`--http-stdio`: in the browser desktop a figure appears inline instead,
under the entry that changed it (see "The command window in a browser").

### Workspace

`clear clc who whos`. `clear()` with no arguments empties the workspace, and
so does `clear all`; `clear('a')` removes one name, and so does the command
form `clear a`. `clc` clears the terminal, and writes nothing when the output
is not a terminal: a pipe, a file, `evalc`, `--protocol` or `--ui`.

```matlab
a = 1;
b = [1 2 3];
c = 'hi';
who
whos
clear('a')
who
clear
who
disp('done')
```

```
Your variables are:

a  b  c

  a            1x1    8  double
  b            1x3   24  double
  c            1x2    4  char

Your variables are:

b  c

done
```

`who` prints the names, and `whos` a row per variable of its size, bytes and
class: 8 bytes a double element, 16 a complex one, 1 a logical, 2 a char,
and a cell or struct the sum of what it holds. The layout is SplatCrab's own
and has no heading row. `clear x`, without parentheses, is
[command syntax](#command-syntax).

### The environment

`cd pwd ls dir help which format eval evalc run datestr now clock pause
getenv system version exit quit`. The current folder belongs to the
interpreter: `cd` moves it, and every path it resolves (files on the path,
the file functions, `ls`, `dir`, `run`, `system`) follows, while the
process's own folder never changes. Under `--protocol`, `--ui` and
`--http-stdio`, `cd` cannot leave the folder the session started in:
`cd ..` in that folder is `Cannot CD to ..: it is outside the file root.`
At the REPL and in a script it goes anywhere. `help name` prints a
builtin's help line, or a file's leading `%` comment block; `which name`
says where a call of `name` goes. `eval(code)` runs text in the workspace,
`v = eval(expr)`
evaluates it, and `evalc(code)` returns what it printed, less the final line
end. `pause(n)` waits `n` seconds, up to a day; a bare `pause` waits for
Enter at a terminal and is an error in a pipe, so a piped script never
hangs. `system(cmd)` runs a shell command in the current folder and returns
its status, and with a second output its text. `exit` and `quit` are
statements anywhere, and `exit(n)` exits with code `n`, from 0 to 255; under
`--protocol` and `--ui` they are an error, since the session is the
client's.

```matlab
eval('q = 6 * 7;')
disp(q)
s = evalc('disp(1)');
disp(numel(s))
which sum
help numel
disp(ischar(pwd))
disp(ischar(datestr(now)))
disp(ischar(version))
disp(1)
exit(3)
disp(2)
```

```
    42
     6
built-in (sum)
numel(A) - the number of elements of A.
   1
   1
   1
     1
```

The script above exits with code 3, and `disp(2)` never runs. Under
`--http-stdio` too `exit` and `quit` are refused, and `try` never catches
an `exit`.

`who` lists the names alone; `whos` adds each variable's size, bytes (8 per
double element, 16 per complex one, 1 per logical, 2 per char) and class,
in SplatCrab's own layout with no heading row. `format long` shows fifteen
decimals, `format short` or a bare `format` goes back to four.

```matlab
x = 1:3;
s = 'ab';
who
whos
format long
disp(x / 3)
format
disp(x / 3)
```

```
Your variables are:

s  x

  s            1x2    4  char
  x            1x3   24  double

   0.333333333333333   0.666666666666667   1.000000000000000
    0.3333    0.6667    1.0000
```

### Functions and the path

`nargin nargout exist feval addpath rmpath`, and `arrayfun func2str str2func`
for handles, described with user functions
under [Functions](#functions).

### Timing

`tic toc`. Bare `tic` sets the mark and bare `toc` reads it; `t = tic` returns
a handle for `toc(t)`. A bare `toc` with no earlier bare `tic` is an error.

```matlab
tic
x = sum(1:1000);
t = toc;
fprintf('elapsed is a number: %d\n', t >= 0);
h = tic;
e = toc(h);
fprintf('handle form works: %d\n', e >= 0);
```

```
elapsed is a number: 1
handle form works: 1
```

```matlab
toc
```

```
Error: Line 1: You must call TIC without an output argument before calling TOC without an input argument.
```

## Argument forms

These are the argument shapes MATLAB code actually uses, added in cycle 01c.
They are the newest part of SplatCrab and the part least covered elsewhere.

### Size vectors and sized constants

`zeros`, `ones`, `eye`, `rand`, `NaN`, `Inf`, `true` and `false` all take a
scalar, two sizes, or a **row** vector of sizes, so `zeros(size(A))` works.

```matlab
A = ones(2, 3);
Z = zeros(size(A))
sz = size(A);
N = NaN(sz)
T = true(2)
F = false(1, 3)
E = eye(size(A))
```

```
Z =

     0     0     0
     0     0     0

N =

   NaN   NaN   NaN
   NaN   NaN   NaN

T =

  2×2 logical array

   1   1
   1   1

F =

  1×3 logical array

   0   0   0

E =

     1     0     0
     0     1     0

```

`pi(2)` stays an error, as in MATLAB. A trailing size of `1` is dropped, so
`zeros(2, 3, 1)` is 2x3, while any other third size makes an [N-D
array](#n-d-arrays), `zeros(2, 3, 0)` an empty 2x3x0. `eye` keeps two
sizes, and `cell` keeps the clean error `N-D arrays are not supported.`
for a third size other than `1`, while `repmat(1, 2, 2, 2)` tiles a
2x2x2. A size past what `usize` holds is named as you asked for it, not
as the clamp:

```matlab
x = zeros(1e300);
```

```
Error: Line 1: Requested 1e+300x1e+300 array exceeds the maximum array size.
```

### `reshape` and `repmat` with a placeholder

`reshape(A, sz)` takes a size vector, and one `[]` stands for the size that
makes the element count come out.

```matlab
A = 1:6;
B = reshape(A, [], 2)
C = reshape(A, 2, [])
D = reshape(A, [3 2])
R = repmat([1 2], [2 2])
```

```
B =

     1     4
     2     5
     3     6

C =

     1     3     5
     2     4     6

D =

     1     4
     2     5
     3     6

R =

     1     2     1     2
     1     2     1     2

```

Two placeholders, or a count that does not divide, are errors.

### `sort` direction

`sort(v, direction)`, `sort(v, dim)` and `sort(v, dim, direction)`.
`direction` is `'ascend'` or `'descend'`. Both are stable; `NaN` goes last
ascending and first descending.

```matlab
v = [3 1 2 NaN];
a = sort(v)
b = sort(v, 'descend')
c = sort([3 1 2], 'ascend')
```

```
a =

     1     2     3   NaN

b =

   NaN     3     2     1

c =

     1     2     3
```

A `NaN` no longer widens its row: it has no digits, so it neither changes the
format nor the column width. The values were always right; only the display
was wrong.

### `find` count and direction

`find(X, n)`, `find(X, n, 'first')` and `find(X, n, 'last')`. The last `n`
come back in ascending order. `n` must be a positive integer.

```matlab
x = [0 1 1 0 1];
a = find(x)
b = find(x, 2)
c = find(x, 2, 'first')
d = find(x, 2, 'last')
```

```
a =

     2     3     5

b =

     2     3

c =

     2     3

d =

     3     5

```

### `norm` order

`norm(v, p)` for `p` of `1`, `2`, any positive real, `Inf`, `-Inf`, `'fro'` or
`'inf'`. The computation scales by the largest magnitude, so it does not
overflow.

```matlab
v = [3 -4];
a = norm(v)
b = norm(v, 1)
c = norm(v, 2)
d = norm(v, Inf)
e = norm(v, -Inf)
f = norm(v, 3)
g = norm(v, 'fro')
h = norm([1e200 1e200])
```

```
a =

     5

b =

     7

c =

     5

d =

     4

e =

     3

f =

    4.4979

g =

     5

h =

  1.4142e+200

```

`p = 0` and a negative finite `p` are refused. Vectors only, still.

### `diag` offset

`diag(v, k)` puts `v` on the k-th diagonal of a square matrix of order
`numel(v) + abs(k)`; `diag(A, k)` pulls the k-th diagonal out as a column.

```matlab
A = diag([1 2], 1)
B = diag([1 2], -1)
M = [1 2 3; 4 5 6; 7 8 9];
c = diag(M, 1)
d = diag(M, -1)
```

```
A =

     0     1     0
     0     0     2
     0     0     0

B =

     0     0     0
     1     0     0
     0     2     0

c =

     2
     6

d =

     4
     8

```

A `k` past the matrix gives a 0x1.

### `num2str` precision and `round` digits

`num2str(x, n)` formats a scalar with `%.{n}g`; `num2str(x, formatSpec)` is
`sprintf` with leading whitespace trimmed. `round(x, n)` rounds to `10^-n` for
any integer `n`, and `round(x, n, 'significant')` to `n` significant digits.

```matlab
a = num2str(pi)
b = num2str(pi, 8)
c = num2str(pi, '%8.4f')
d = round(pi, 2)
e = round(pi, 4)
f = round(12345, -2)
g = round(pi, 3, 'significant')
h = round(12345, 2, 'significant')
```

```
a =

    '3.1416'

b =

    '3.1415927'

c =

    '3.1416'

d =

    3.1400

e =

    3.1416

f =

       12300

g =

    3.1400

h =

       12000

```

`round(x, n, 'decimals')` is the two-argument form spelled out. `num2str` of a
non-scalar gives one char row per matrix row since cycle 11; see
[Strings and text](#strings-and-text).

### `'all'` on the reductions

`sum`, `prod`, `mean`, `any` and `all` take `'all'`; `max` and `min` take it
as their third argument after an empty second.

```matlab
A = [1 2; 3 4];
a = sum(A, 'all')
b = mean(A, 'all')
c = any(A > 3, 'all')
d = all(A > 0, 'all')
e = max(A, [], 'all')
f = min(A, [], 'all')
g = max(A, 3)
```

```
a =

    10

b =

    2.5000

c =

  logical

   1

d =

  logical

   1

e =

     4

f =

     1

g =

     3     3
     3     4

```

A char argument is never read as a dimension, so `sum(A, 'x')` is an error
rather than a silent reduction along dimension 120.

### `eps(x)`

The spacing at `abs(x)`, elementwise. `eps('double')` is plain `eps`;
`eps('single')` waits for a `single` class, which no cycle schedules yet.

```matlab
a = eps
b = eps(1)
c = eps(0)
d = eps(1e308)
e = eps('double')
f = eps([1 1000])
g = eps(Inf)
```

```
a =

   2.2204e-16

b =

   2.2204e-16

c =

  4.9407e-324

d =

  1.9958e+292

e =

   2.2204e-16

f =

   1.0e-13 *

    0.0022    1.1369

g =

   NaN

```

### Smaller argument fixes

`linspace` floors a fractional count; `dot` handles matrices column-wise and
takes a dimension; `max` and `min` of an empty follow MATLAB's shape rule.

```matlab
a = linspace(0, 1, 2.7)
b = size(linspace(0, 1, 0.5))
A = [1 2; 3 4];
B = [5 6; 7 8];
c = dot(A, B)
d = dot(A, B, 2)
e = dot([1 2 3], [4; 5; 6])
```

```
a =

     0     1

b =

     1     0

c =

    26    44

d =

    17
    53

e =

    32

```

```matlab
a = size(max(zeros(3, 0)))
b = size(max(zeros(0, 3)))
c = any(NaN)
d = all(NaN)
e = isvector(zeros(1,0))
f = isvector(zeros(0,0))
g = size(zeros(2, 3, 1))
```

```
a =

     1     0

b =

     0     3

c =

  logical

   0

d =

  logical

   1

e =

  logical

   1

f =

  logical

   0

g =

     2     3

```

## Output and formatting

### `disp` and automatic display

`disp` prints the value with no name and no blank lines. Automatic display
prints `name =`, a blank line, the value, and a blank line.

```matlab
disp('text')
disp(42)
disp([1 2; 3 4])
disp([1.5 2.5])
x = 42
y = [1.5 2.5]
```

```
text
    42
     1     2
     3     4
    1.5000    2.5000
x =

    42

y =

    1.5000    2.5000

```

An all-integer matrix prints in integer columns, twelve wide once a value
reaches 1000. Anything else prints in fixed point with four decimals when its
largest magnitude is from 0.01 up to 1000, with an exact zero shown as a bare
`0`. Outside that range a scalar switches to short exponential form and a
matrix is printed under a common scale factor such as `1.0e+03 *`. An empty
shows its typed header, except the 0x0 `[]`. Wide matrices wrap into MATLAB's
`Columns N through M` blocks, at 80 columns. `format long` keeps every one
of these rules and writes fifteen decimals instead of four; `format short`,
or `format` alone, goes back.

```matlab
a = 1000
b = [1 1000]
c = [0 1.5]
d = [1.5 1000.5]
e = [0.001 0.002]
f = 1234.5
g = 1e10
h = zeros(0, 3)
```

```
a =

        1000

b =

           1        1000

c =

         0    1.5000

d =

   1.0e+03 *

    0.0015    1.0005

e =

   1.0e-03 *

    1.0000    2.0000

f =

   1.2345e+03

g =

   1.0000e+10

h =

  0×3 empty double matrix

```

A logical displays four wide under a `logical` or `R×C logical array` header,
and a char with other than one row under `R×C char array`; see
[Classes](#classes). The `×` in a header is U+00D7, and on Windows SplatCrab
switches the console to UTF-8 so that it renders.

### `fprintf` conversions

`%d %i %u %f %F %e %E %g %G %x %X %o %c %s`, with flags, width and
precision.

```matlab
fprintf('%d %i %u\n', 7, 8, 9);
fprintf('%f\n', pi);
fprintf('%e\n', pi);
fprintf('%g\n', 0.0001);
fprintf('%g\n', 123456789);
fprintf('%c%c%c\n', 72, 105, 33);
fprintf('%s\n', 'hello');
fprintf('%s\n', 65);
```

```
7 8 9
3.141593
3.141593e+00
0.0001
1.23457e+08
Hi!
hello
A
```

`%s` of a number prints its character, and `%c` does too. A `%d` given a
non-integer switches to `%e`, which is MATLAB's rule:

```matlab
fprintf('%d\n', 2.5);
fprintf('%d\n', 3);
n = num2str(42)
m = num2str(1.5)
```

```
2.500000e+00
3
n =

    '42'

m =

    '1.5'

```

### Flags, width and precision

`-` left-justifies, `+` forces a sign, a space reserves a column for one, `0`
pads with zeros, and a precision on an integer is a minimum digit count.

```matlab
fprintf('[%8.3f]\n', pi);
fprintf('[%-8.3f]\n', pi);
fprintf('[%+d]\n', 5);
fprintf('[% d]\n', 5);
fprintf('[%05d]\n', 42);
fprintf('[%.3d]\n', 7);
fprintf('[%10s]\n', 'hi');
fprintf('[%-10s]|\n', 'hi');
```

```
[   3.142]
[3.142   ]
[+5]
[ 5]
[00042]
[007]
[        hi]
[hi        ]|
```

The `#` flag keeps the point, `0` never pads `Inf` or `NaN` with zeros,
`%x`, `%X`, `%o` and a `*` width or precision exist, and the escapes
`\xN`, `\N` (octal), `\a`, `\b`, `\f` and `\v` are processed (cycle 11).
`%E` and `%G` write an upper-case `E`, and `%s` of a number that is not a
character code is `%e`. An invalid conversion ends the output, the text
before it printed, as MATLAB does:

```matlab
fprintf('%x %X %o\n', 255, 255, 8);
fprintf('[%*d] [%.*f]\n', 5, 42, 2, pi);
fprintf('%E %G\n', 12345.678, 1e-10);
fprintf('[%#.0f] [%05d]\n', 3, -Inf);
fprintf('\x41\102\n');
fprintf('%s\n', pi);
fprintf('abc%q def\n', 1);
fprintf('\n');
fprintf(1, 'to stdout\n');
```

```
ff FF 10
[   42] [3.14]
1.234568E+04 1E-10
[3.] [ -Inf]
AB
3.141593e+00
abc
to stdout
```

A width or precision past 8192, written or given by `*`, is an error.

### Format cycling

The format repeats until the arguments run out. A matrix argument is consumed
in column-major order, and the whole argument list is flattened into one
stream.

```matlab
v = [1 2 3];
fprintf('%d\n', v);
fprintf('%d %d\n', v);
```

```
1
2
3
1 2
3 
```

The second call stops mid-format when the values run out, dropping the rest of
the template — MATLAB does the same. Note the last line is `3 ` with a
trailing space and no newline, because the `\n` was never reached.

```matlab
A = [1 2; 3 4];
fprintf('%d ', A);
fprintf('\n');
x = sprintf('%d,', 1:5)
```

```
1 3 2 4 
x =

    '1,2,3,4,5,'

```

### `sprintf` and escapes

`sprintf` returns the string instead of printing it. `\n` `\t` `\\` and `%%`
are processed.

```matlab
s = sprintf('%d + %d = %d', 2, 3, 5)
t = sprintf('%.2f', pi);
disp(t)
u = sprintf('a\tb\nc');
disp(u)
fprintf('%d%%\n', 50);
fprintf('a\\b\n');
```

```
s =

    '2 + 3 = 5'

3.14
a	b
c
50%
a\b
```

## Errors and exit codes

A script that fails prints `Error: Line N: <message>` to stderr and exits 1.
stdout is flushed first, so everything that ran before the failure is visible.
The exit code is what a caller can rely on: 0 when the script ran to its end
or to a bare `exit`, 1 for an error, `n` for `exit(n)`, and the REPL the same
at its end. Any other code, 101 for a panic or 134 for an abort, is a bug in
SplatCrab.

```matlab
disp('before')
x = undefined_thing + 1;
disp('after')
```

```
before
Error: Line 2: Unrecognized function or variable 'undefined_thing'.
```

The line is the line of the statement that raised the error, including inside
a loop or an `if` body:

```matlab
for k = 1:3
    disp(k)
    y = k / nope;
end
```

```
     1
Error: Line 3: Unrecognized function or variable 'nope'.
```

Common messages:

```matlab
A = [1 2 3];
B = [1 2];
C = A + B;
```

```
Error: Line 3: Arrays have incompatible sizes for operator '+' (1x3 vs 1x2).
```

```matlab
v = [1 2 3];
v(0)
```

```
Error: Line 2: Index in position 1 is invalid. Array indices must be positive integers or logical values.
```

```matlab
v = [1 2 3];
v(7)
```

```
Error: Line 2: Index exceeds the number of array elements. Index must not exceed 3.
```

`error` raises your own message, and takes `sprintf` arguments:

```matlab
error('value %d is too big', 7)
```

```
Error: Line 1: value 7 is too big
```

With one argument the message is literal, with no format or escape
processing, as in MATLAB:

```matlab
error('100% sure')
```

```
Error: Line 1: 100% sure
```

A first argument with a colon and no whitespace, followed by more arguments,
is an identifier, which a `catch` reads back as `e.identifier`:
`error('MyPkg:myid', 'Value %d bad', 7)` reports `Value 7 bad`. When every
argument is empty, `error` raises nothing:

```matlab
error('')
disp('still running')
```

```
still running
```

`warning` takes the same arguments and prints `Warning: <message>` to stderr,
and the script goes on; the exit code stays 0. Under `--protocol` and `--ui`
the warning is part of the `eval`'s `out`, where it was raised.

```matlab
disp('before')
warning('careful %d', 1)
```

```
before
Warning: careful 1
```

`assert(cond)` raises `Assertion failed.` when `cond` does not hold by the
rule `if` uses, and `assert(cond, fmt, ...)` raises that message instead:

```matlab
assert(1 + 1 == 2)
assert(1 + 1 == 3, 'arithmetic is off by %d', 1)
```

```
Error: Line 2: arithmetic is off by 1
```

A script is parsed in full before anything runs, so a syntax error anywhere
means nothing executes:

```matlab
x = 1
y = x + ;
```

```
Error: Line 2: unexpected ';' in expression
```

The line is right, and the token is named as it is written, `';'`. It used to
print the internal `Debug` name, `Semi`.

### Exit codes

| Code | Meaning |
|---|---|
| 0 | The script ran to the end |
| 1 | A lex, parse or runtime error; the message is on stderr |
| 101 | The interpreter panicked — always a bug, please report it |
| 134 | The process was aborted: the allocator refused, or a stack overflowed |

Exit 101 is reserved for panics so a crash can never be mistaken for a clean
error, and nothing you are likely to type reaches it. An absurd format field,
a result too large to allocate, and a size that would overflow are all
ordinary errors:

```matlab
fprintf('%.65536f\n', 1);
```

```
Error: Line 1: The width or precision in a format specifier must be at most 8192.
```

No input the project knows of reaches 101 or 134 any more. Nesting is bounded
at 10,000 levels in both the parser and the evaluator, so an over-deep
expression is an ordinary error rather than a stack overflow. That is a claim
about every input tried, not a proof; if you find one that crashes, it is a
bug worth reporting.

## Differences from MATLAB

Everything here is verified against this binary. It is not the complete bug
list — `docs/ARCHITECTURE.md` has the "Known deviations" and "Known bugs"
tables with the cycle each is scheduled to — but it is the set you are most
likely to hit.

### Display

Cycle 02 brought the display in line with MATLAB's rules: a bare `0`, typed
empties, wide integer columns, the common scale factor and short exponential
form, shown under [`disp` and automatic display](#disp-and-automatic-display).
Those rules were written from recorded MATLAB output rather than from a run of
MATLAB, so a case they do not cover may still differ. One known difference is
left, and it is in the value rather than the display: `det` of this integer
matrix lands exactly on `-2`.

```matlab
a = [0 1.5]
b = []
c = [1000 2000]
d = [1000.5 2000.5]
e = det([1 2; 3 4])
f = 1234.5
g = 0.001
h = 1e10
```

```
a =

         0    1.5000

b =

     []

c =

        1000        2000

d =

   1.0e+03 *

    1.0005    2.0005

e =

    -2

f =

   1.2345e+03

g =

   1.0000e-03

h =

   1.0000e+10

```

MATLAB gives `-2.0000` for `e`: its determinant is a roundoff away from `-2`,
and a value that is not a whole number prints with decimals here too. Cycle
08's shared LU kept the elimination order `det` had, rather than choosing one
that would produce MATLAB's digits, so the difference stays until a source
settles LAPACK's order.

A `NaN` or `Inf` in a row keeps MATLAB's integer columns, which it did not
before cycle 01e:

```matlab
disp([1 2 NaN])
disp(NaN)
```

```
     1     2   NaN
   NaN
```

MATLAB gives the same: it keeps integer columns when the only non-integer
entries are non-finite.

The colon operator used to miss its end point here, so `0:0.1:0.3` did not
finish on `0.3`, and `0:Inf` gave an empty rather than being refused. Both are
fixed: the range is computed from the right-hand end so it lands exactly, and
an infinite end point is an error.

### Logical values

Comparisons return logicals, and a logical subscript is a mask, as in MATLAB.
Before cycle 02 a mask was a double of ones and zeros and `x(x > 0)` read it
as a list of positions, giving `5 5 5` with no error; from cycle 02 to cycle
03 it was a clean error. It now selects, including a mask with no zeros:

```matlab
x = [5 6 7];
y = x(x > 0)
x(x > 0) = 0
```

```
y =

     5     6     7

x =

     0     0     0

```

`NaN` cannot be converted to a logical, and `&&` and `||` require an operand
convertible to a logical scalar. Both refuse, as MATLAB does:

```matlab
if NaN, disp('NaN is true'), end
a = NaN & 1
b = ~NaN
c = [1 1] && 1
```

```
Error: Line 1: NaN's cannot be converted to logicals.
```

### Indexing

- A second `(...)` indexes the value so far, so `x(2:3)(2)` and `size(A)(2)`
  work, as in Octave; MATLAB refuses to chain parentheses. `x()` with no
  subscripts is `Only 1-D and 2-D indexing is supported.`, where MATLAB
  returns `x`.
- An N-D array grows only through as many subscripts as it has dimensions,
  or more: a linear subscript past its end, and a subscript past the end of
  the fold that fewer subscripts index, are `Attempt to grow array along
  ambiguous dimension.`
- The builtins not yet taught N-D arrays refuse one by name, `N-D arrays are
  not supported by 'num2str'.`, where MATLAB takes most of them; so does
  `save -ascii`. `horzcat` and `vertcat`, like `cat`, take arrays only, and
  `flip`, `circshift`, `fliplr` and `flipud` refuse a cell, where MATLAB
  takes one. The per-page class line of a
  logical or char N-D array, the `(:,:,k) =` headers of `disp` and the
  `2×0×3 empty double array` wording are SplatCrab's stated rules, not
  checked against MATLAB.
- A cs-list is not spread into index subscripts, so `x(c{:})` of a
  two-element `c` is the cs-list error, and `[c{:}] = deal(0)` is not a
  target list; MATLAB accepts both.
- `A(:, :) = []` leaves an empty with no rows and the original columns;
  that has not been checked against MATLAB.
- `[m, i] = max(a, b)`, the two-array form, is `Too many output arguments.`

```matlab
x = [1 2 3];
x(2:3)(2)
x()
```

```
ans =

     3

Error: Line 3: Only 1-D and 2-D indexing is supported.
```

### Numerics

`matmul` used to skip a multiply when one factor was zero, which swallowed
`Inf` and `NaN`. It no longer does:

```matlab
a = [Inf 0] * [0; 1]
```

```
a =

   NaN
```

A result that would be complex was refused from cycle 01d, rather than
returned as a `NaN` that looks computed; since cycle 10 it is the complex
value:

```matlab
sqrt(-4)
```

```
ans =

   0.0000 + 2.0000i

```

`trace([])` used to print `-0`, and `%d` used to saturate at 2^63. Both are
fixed, and both now agree with MATLAB:

```matlab
fprintf('%.4f\n', trace([]));
fprintf('%d\n', 1e30);
```

```
0.0000
1000000000000000019884624838656
```

### Char class

Indexed assignment, growth, concatenation and the rearrangement builtins keep
a char, and unary plus gives a double, as in MATLAB; see [Classes](#classes).
Since cycle 11 a range between two chars and `diag` of a char keep it too:

```matlab
r = 'a':'e'
d = diag('ab');
disp(class(d))
disp(size(d))
```

```
r =

    'abcde'

char
     2     2
```

`logical('a')` and `char(true)` convert here, where MATLAB is understood to
refuse both.

### Syntax not recognised

Command syntax decides whether a name is a variable before the script runs,
from the names the script has assigned by then; a name cleared and then used
in command form stays an expression. Chained ranges parse as
MATLAB reads them,
`1:2:3:4` as `(1:2:3):4`, but a colon operand that is not a scalar is then an
error. Hex literals (`0x1F`) are rejected. A UTF-8 byte-order mark at the
start of a file is skipped; a UTF-16 file is not read.

A `break` or `continue` outside a loop is an error, as in MATLAB. It used to
end the script silently with exit 0:

```matlab
disp(1)
break
disp(2)
```

```
     1
Error: Line 2: 'break' is only valid inside a loop.
```

`2` never prints and the exit code is 1.

### Builtin behaviour

The numerics of cycle 09 differ in a few recorded ways: a solver that
fails (`fzero` with no sign change, `fminsearch` at its cap, a divergent
`integral`, an `ode45` below its smallest step) is an error, where MATLAB
warns and returns what it has; `polyfit` with fewer distinct points than
coefficients warns with the rank-deficient text of `\`; and `ode45` asked
for one output gives a struct with `solver`, `x` and `y` only. The Design
notes of `docs/modules/09-numerics.md` list every choice.

A system that is singular only to
working precision warns with MATLAB's "singular" text, where MATLAB's is "close
to singular or badly scaled" with an `RCOND`, and a rank-deficient
least-squares system warns with SplatCrab's own text. `eig`, `svd` and the
builtins over `svd` refuse `NaN` and `Inf`. `who` prints the typed table that
is MATLAB's `whos`.

A caught error is a minimal `MException`: `e.message`, `e.identifier`,
`e.stack` and `class(e)`, with no `cause`, and a one-line display of
SplatCrab's own. `e.stack` lists the function frames only, not the script's
own. An error the interpreter raises itself has an empty
identifier, where MATLAB's carry one such as `MATLAB:UndefinedFunction`.
`warning('off')` and `lastwarn` do not exist: `warning('off')` prints
`Warning: off`.

Empty-result shapes match MATLAB: `find([])` and `diag([])` are 0x0,
`size('')` is `0 0`, and `disp([])` prints nothing. All four differed before
cycle 01e. Since cycle 14c `find(0)` is 0x0 too, by the `find` page's
convention for a scalar zero, where it was 1x0.

```matlab
a = size(find([]))
b = size(diag([]))
c = size('')
disp([])
```

```
a =

     0     0

b =

     0     0

c =

     0     0

```

### Strings and files

The string and file functions of cycle 11 differ in these recorded ways,
each safer or simpler than MATLAB's: `delete` refuses a wildcard rather
than expand it; the regular-expression engine refuses backreferences,
lookaround, atomic groups, possessive quantifiers, conditionals and inline
flags, and there is no `regexpi`; `str2num` reads literals and operators
and never hands its text to `eval`, so text from a file cannot run code;
`input` of text that is not an expression is an error, where MATLAB asks
again; `save` in an empty workspace is an error; a compressed MAT-file is
refused, and the integer and `single` classes load as doubles; `fopen`
takes no machine format or encoding, and text is UTF-8 both ways.

```matlab
[x, ok] = str2num('disp(1)')
y = str2num('[1 2] * 3')
```

```
x =

     []

ok =

  logical

   0

y =

     3     6

```

### Plotting

`gcf` and `figure` give the figure's number, a double, as MATLAB did
before R2014b; SplatCrab has no graphics objects, so there are no handles
to set properties through, and no builtin returns one. A complex argument
to a plotting builtin is refused, where MATLAB plots the real part against
the imaginary. Labels are plain text, with no TeX. `histogram` with no bin
count uses Sturges' rule, and the tick rule, the fonts and the layout are
SplatCrab's own; a figure cannot be zoomed, panned or clicked, and there
are no 3-D plots.

### Error text

Message text follows MATLAB R2020a and later, except where SplatCrab's wording
says strictly more. An unknown name is `Unrecognized function or variable
'x'.`, which is R2020a's phrasing rather than the older `Undefined function or
variable`.

One message deliberately keeps its own wording, because it says more than
MATLAB's: the size-mismatch message names the operator and both shapes. MATLAB
says only `Arrays have incompatible sizes for this operation.` The `x(0)`
message used to end `must be positive integers.`; since logical indexing
arrived in cycle 03 it ends `must be positive integers or logical values.`,
as MATLAB's does.

### Functions

`exist` gives `5` for every builtin, where MATLAB gives `2` for the ones it
ships as `.m` files, `linspace` among them. What `exist` gives for a
function local to the running script is not settled by MATLAB's
documentation; SplatCrab gives `0`. The trace names a local function or a
subfunction alone, `in g3`, where MATLAB writes `script>g3`. A function in a
file on the path can call a local function of the script being run, which
MATLAB does not allow. A file may mix functions that end with `end` and
functions that do not, and a script's local functions may go without `end`;
MATLAB refuses both. Calling a script with arguments or for a value is
`Too many input arguments.` or `Too many output arguments.`, where MATLAB
names the script in its message.

### REPL

REPL diagnostics used to go to stdout rather than stderr, so a piped session could not
separate them from output. An incomplete block at end of input is discarded
silently.

## Not yet

None of the following exist. `docs/ROADMAP.md` gives the order.

| Missing | Arrives in |
|---|---|
| `global`, `persistent`, nested functions | later |
| The rest of the builtins on N-D arrays: `num2str`, `mat2str`, the strings, the sets and the linear algebra | later |
| N-D cell and struct arrays | later |
| Compressed MAT-files, `regexpi`, backreferences | not scheduled |
| 3-D plots, surfaces, interaction with a figure | not scheduled |
| Integer classes and `single` | not scheduled |

Hitting one of these gives a parse error or another clean error, never a
wrong answer:

```matlab
num2str(zeros(2, 3, 4))
```

```
Error: Line 1: N-D arrays are not supported by 'num2str'.
```
