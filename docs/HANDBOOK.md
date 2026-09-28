# SplatCrab Handbook

How to use SplatCrab, for someone who already knows MATLAB.

SplatCrab is a MATLAB-compatible numerical language written in Rust. It runs
`.m` scripts and gives you a REPL. Every value is a matrix stored
column-major, exactly as MATLAB stores it, so linear indexing, `reshape` and
`(:)` agree with MATLAB element for element. A matrix is of class `double`,
`logical` or `char`, as in MATLAB.

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
- [Control flow](#control-flow)
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
the prompt open until it is closed. `exit` or `quit` on a line by itself
leaves.

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

There are four operations. `eval` runs `code` as a REPL entry, possibly
several lines, and answers with the output it would have printed (`out`)
and, if it failed, an `error` holding the REPL's message and the one-based
line within `code`. `complete` answers whether `code` is a finished entry or
still inside an open block or bracket, the same test the REPL uses to decide
whether to keep reading. `workspace` lists each variable's name, size and
class, sorted by name. `completions` lists every variable and builtin whose
name starts with `prefix`. Every response starts with the request's `id` (a
number or a string, or `null` when it sent none) and `ok`.

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
{"id":5,"ok":true,"items":["diag","disp"]}
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

Every number is an IEEE 754 double. There are no integer classes, no `single`,
and no complex numbers yet. Note `w`: a scalar that is not a whole number
switches to short exponential form below 0.01 and from 1000 up, as MATLAB's
does. [`disp` and automatic display](#disp-and-automatic-display) gives the
rules.

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

`%{ ... %}` block comments are **not** recognised: the lines between them
execute. See [Differences](#differences-from-matlab).

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

Backslash solves a linear system, and `/` is its right-hand twin. **Only
square systems are supported**: a non-square `A\b` is an error, not a
least-squares solution.

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

Transpose is `'`, and `.'` is the same thing since there are no complex
numbers. The lexer decides between transpose and a string delimiter by what
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

Anything past 1 there is out of bounds:

```matlab
A = [1 2; 3 4];
A(1, 1, 2)
```

```
Error: Line 2: Index in position 3 exceeds array bounds. Index must not exceed 1.
```

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

Cell arrays and structs, which do support them, arrive in cycle 07.

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

## Builtins

88 names, each an ordinary function registered by name. Every one rejects
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

`size numel length isempty isscalar isvector`. `size(A)` returns the row
vector `[rows cols]`, `size(A, d)` one dimension. `length` is the largest
dimension.

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

### Rearrangement

`reshape repmat fliplr flipud`. `reshape` fills column-major, like MATLAB.

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

A dimension past the array's leaves the input unchanged; `0` is an error. A
char is never read as a dimension. `any` ignores `NaN`, so `any(NaN)` is `0`
and `all(NaN)` is `1`, matching the MATLAB page.

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

`round` breaks ties away from zero, as MATLAB does. Anything whose real answer
would be complex — `sqrt(-4)`, `log(-1)`, `asin(2)` — is a clean error until
complex numbers arrive in cycle 10; see [Numerics](#numerics).

### Predicates

`isnan isinf isfinite`, beside the shape queries `isempty isscalar isvector`
and the class tests `islogical ischar isnumeric isa`. Every one returns a
logical.

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

`transpose inv det trace diag norm dot`. `norm` takes **vectors only** until
cycle 08 — `norm` of a matrix is an error, which is the easiest mistake to
make in this section.

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

```matlab
n = norm([1 2; 3 4]);
```

```
Error: Line 1: 'norm' currently supports vectors only.
```

A singular `inv` is an error rather than MATLAB's warning plus `Inf`:

```matlab
A = [1 2; 2 4];
b = inv(A);
```

```
Error: Line 2: Matrix is singular to working precision.
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

`find sort`. Like `norm`, `sort` is vectors only until cycle 09.

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

```matlab
s = sort([3 1; 2 4]);
```

```
Error: Line 1: 'sort' currently supports vectors only.
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

`disp fprintf sprintf num2str error`. See
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

### Workspace

`clear clc who whos`. `clear()` with no arguments empties the workspace;
`clear('a')` removes one name. `clc` writes the ANSI clear-screen sequence.

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

  a            1x1 double
  b            1x3 double
  c            1x2 char

Your variables are:

  a            1x1 double
  b            1x3 double
  c            1x2 char

Your variables are:

  b            1x3 double
  c            1x2 char

done
```

Two notes. `who` prints the typed table that MATLAB's `whos` prints, and
`whos` prints the same thing; and `clear x` in command syntax is a parse
error, so the parentheses are required:

```matlab
x = 1;
clear x
```

```
Error: Line 2: unexpected 'x'
```

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
`zeros(2, 3, 1)` is 2x3; any other third size, including `0`, is the clean
error `N-D arrays are not supported.` A size past what `usize` holds is named
as you asked for it, not as the clamp:

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
non-scalar still returns one row in column-major order; that waits for cycle
11.

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
`Columns N through M` blocks, at 80 columns. There is no `format long` or
`format short` yet.

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

`%d %i %u %f %e %g %c %s`, with flags, width and precision.

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

Precision on `%s` is currently ignored, `#` is ignored, `%x` `%X` `%o` and a
`*` width are errors, and `\x` `\a` `\b` `\f` `\v` and octal escapes are not
processed. A very large width or precision panics; see
[Errors and exit codes](#errors-and-exit-codes).

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

A script is parsed in full before anything runs, so a syntax error anywhere
means nothing executes:

```matlab
x = 1
y = x + ;
```

```
Error: Line 2: unexpected ';' in expression
```

The line and position are right, but the token is named by its internal
token's own spelling, `';'`, which is what it does now. It used to print the
internal `Debug` name, `Semi`.

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
and a value that is not a whole number prints with decimals here too. Cycle 08
rewrites `det`.

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
- There are no N-D arrays, so `A(:, :, [1 1])`, `A(:, :, [])` and growth into
  a second page (`A(1, 1, 2) = 5`) are `N-D arrays are not supported.`, where
  MATLAB builds the N-D array.
- `s.a = 1` on an undefined `s` is the Dot error rather than a new struct,
  until cycle 07.
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

A result that would be complex is refused rather than returned as a `NaN` that
looks computed. MATLAB gives `0 + 2.0000i` for the first of these, and
`0 + 3.1416i` and `1.0000 + 1.7321i` for `log(-1)` and `(-8)^(1/3)`. Until
complex numbers arrive in cycle 10, the refusal is the honest answer:

```matlab
sqrt(-4)
```

```
Error: Line 1: Complex results are not supported. 'sqrt' of a negative number is complex.
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
Two builtins still lose the class, because cycle 02 named only the
rearrangements:

```matlab
r = 'a':'c'
d = diag('ab')
```

```
r =

    97    98    99

d =

    97     0
     0    98

```

MATLAB gives `'abc'` and a 2x2 char. `logical('a')` and `char(true)` convert
here, where MATLAB is understood to refuse both.

### Syntax not recognised

Block comments execute their contents:

```matlab
%{
disp(111)
%}
disp(222)
```

```
   111
   222
```

Command syntax is a parse error, so `clear x`, `format long` and `disp hello`
all fail; use `clear('x')`. Chained ranges parse as MATLAB reads them,
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

`norm` and `sort` take vectors only (cycles 08 and 09). Backslash solves
square systems only and errors where MATLAB warns and returns a least-squares
answer. `inv` of a singular matrix errors where MATLAB warns and returns
`Inf`. `who` prints the typed table that is MATLAB's `whos`.

`error` with a single argument applies format processing, which MATLAB does
not:

```matlab
error('100% sure')
```

```
Error: Line 1: 100ure
```

MATLAB reports `100% sure`. With a format and arguments (`error('value %d',
7)`) SplatCrab is correct; it is the one-argument case that is wrong. An
identifier first argument is not supported either.

Empty-result shapes match MATLAB: `find([])` and `diag([])` are 0x0,
`size('')` is `0 0`, and `disp([])` prints nothing. All four differed before
cycle 01e.

```matlab
a = size(find([]))
b = size(diag([]))
c = size('')
disp([])
d = num2str([1 2; 3 4])
```

```
a =

     0     0

b =

     0     0

c =

     0     0

d =

    '1  3  2  4'
```

`num2str` of a matrix gives one row in column-major order; MATLAB gives a
2x4 char array of `'1  2'` and `'3  4'`.

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

### REPL

REPL diagnostics used to go to stdout rather than stderr, so a piped session could not
separate them from output. An incomplete block at end of input is discarded
silently.

## Not yet

None of the following exist. `docs/ROADMAP.md` gives the order.

| Missing | Arrives in |
|---|---|
| `switch` / `case` / `otherwise` | 04 |
| `try` / `catch` | 04 |
| Command syntax (`clear x`, `format long`) | 04 |
| Block comments `%{ ... %}` | 04 |
| `warning` | 04 |
| User-defined functions, `nargin` / `nargout`, scoping | 05 |
| Function handles and anonymous functions `@(x) ...` | 06 |
| Cell arrays `{...}` | 07 |
| Structs `s.field` | 07 |
| Matrix `norm`, least squares, `\` of a non-square system | 08 |
| Matrix `sort`, and `[s, i] = sort(...)` of a matrix | 09 |
| Complex numbers, `1i`, `real`, `imag`, `abs` of a complex | 10 |
| N-D arrays `zeros(2, 3, 4)` | not scheduled |
| `fprintf(fid, ...)`, `nbytes = fprintf(...)`, string functions | 11 |
| Regular expressions, file I/O, `save` / `load` | 11 |
| Plotting | 12 |
| `exit` in a script, `exit(code)`, `format`, `help`, `eval` | 13 |
| Integer classes and `single` | not scheduled |

Hitting one of these gives a parse error or another clean error, never a
wrong answer:

```matlab
f = @(x) x + 1
```

```
Error: Line 1: unexpected '@' in expression
```

```matlab
plot(1:10)
```

```
Error: Line 1: Unrecognized function or variable 'plot'.
```
