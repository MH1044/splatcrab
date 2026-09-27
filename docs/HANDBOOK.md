# SplatCrab Handbook

How to use SplatCrab, for someone who already knows MATLAB.

SplatCrab is a MATLAB-compatible numerical language written in Rust. It runs
`.m` scripts and gives you a REPL. Everything is a double-precision matrix
stored column-major, exactly as MATLAB stores it, so linear indexing,
`reshape` and `(:)` agree with MATLAB element for element.

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
to stdout, not stderr; see [Differences](#differences-from-matlab).

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

    0.0010

v =

   250

```

Every number is an IEEE 754 double. There are no integer classes, no `single`,
and no complex numbers yet. Note `w`: MATLAB prints `1.0000e-03` there;
SplatCrab's scalar display does not yet switch to exponential form.

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

     1     1     1

b =

     1     1     0

c =

    'ell'

d =

   104   101   108   108   111

```

Arithmetic on a char gives numbers, as in MATLAB. The reverse does not hold:
several operations silently lose the char class. See
[Differences](#differences-from-matlab).

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

    0.0000    0.2500    0.5000    0.7500    1.0000

v =

     []

```

Two caveats. A fractional-step range does not land exactly on its end point
the way MATLAB's does (`x = 0:0.1:0.3; x(end) == 0.3` is `0` here, `1` in
MATLAB), and an infinite end point such as `0:Inf` gives an empty where MATLAB
refuses it. Both are in [Differences](#differences-from-matlab).

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

    1.0000    0.0000
    0.0000    1.0000

```

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

`==  ~=  <  <=  >  >=` compare element by element and return 0/1. They return
**doubles**, not logicals — the logical class arrives in cycle 02 — which has
a real consequence for masking, described in
[Differences](#differences-from-matlab).

```matlab
v = [1 5 3];
a = v > 2
b = v == 3
c = v ~= 3
d = v <= 3
```

```
a =

     0     1     1

b =

     0     0     1

c =

     1     1     0

d =

     1     0     1

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

     1     0     0

b =

     1     0     1

c =

     0     1     0

d =

     1

e =

     1

```

Short-circuiting itself is correct, but `&&` and `||` currently accept
non-scalar and empty operands instead of erroring, and `NaN` converts silently
to true. See [Differences](#differences-from-matlab).

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

     1

h =

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

Two forms are **not** available yet: logical indexing (`x(x > 0)` is read as a
list of numeric indices and gives the wrong answer — see
[Differences](#differences-from-matlab)) and deletion (`v(2) = []` is a clean
error).

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

A `break` or `continue` outside a loop currently ends the script silently with
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

`NaN` is the exception to MATLAB agreement: `if NaN` is true here, where
MATLAB refuses the conversion.

## Builtins

80 names, each an ordinary function registered by name. Every one rejects
arguments it does not understand with `Too many input arguments.` rather than
ignoring them. A builtin that produces no value (`disp`, `fprintf`, `clc`,
`clear`, `who`, bare `tic`, bare `toc`) is legal as a statement and is
`Too many output arguments.` in an expression.

### Constants

`pi Inf inf NaN nan eps true false`. `true` and `false` are 0/1 doubles for
now; `eps` alone is the spacing at 1.

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

     1

g =

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

    0.0000    0.2500    0.5000    0.7500    1.0000

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

     0

e2 =

     1

sc =

     1

vv =

     1

```

`[r, c] = size(A)` does **not** work: multiple assignment is unsupported, and
even `[x] = size(A, 1)` is `invalid assignment target`. Call `size` twice.

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

These four lose the char class: `fliplr('abc')` gives numbers, not `'cba'`.

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

     1

f =

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
would be complex — `sqrt(-4)`, `log(-1)`, `asin(2)` — gives `NaN` and exit 0,
which is wrong; see [Differences](#differences-from-matlab).

### Predicates

`isnan isinf isfinite`.

```matlab
a = isnan([1 NaN Inf])
b = isinf([1 NaN Inf])
c = isfinite([1 NaN Inf])
```

```
a =

     0     1     0

b =

     0     0     1

c =

     1     0     0

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
Error: Line 2: unexpected Ident("x")
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

       NaN       NaN       NaN
       NaN       NaN       NaN

T =

     1     1
     1     1

F =

     0     0     0

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

    1.0000    2.0000    3.0000       NaN

b =

       NaN    3.0000    2.0000    1.0000

c =

     1     2     3

```

The four-decimal columns in `a` and `b` are a display bug, not a value bug:
one non-finite element currently forces the whole row out of integer format.
MATLAB prints `1 2 3 NaN`.

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

     1

d =

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
`eps('single')` waits for the classes of cycle 02.

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

   2.2204e-16   1.1369e-13

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

     0

d =

     1

e =

     1

f =

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

An all-integer matrix prints in integer columns; anything else prints in fixed
point with four decimals. There is no `format long` or `format short`, and no
common scale factor (`1.0e+03 *`). Wide matrices print on one long unwrapped
line rather than MATLAB's `Columns 1 through 13` blocks.

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
Error: Line 2: Undefined function or variable 'undefined_thing'.
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
Error: Line 3: Undefined function or variable 'nope'.
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
Error: Line 2: Index in position 1 is invalid. Array indices must be positive integers.
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
Error: Line 2: unexpected Semi in expression
```

The line and position are right, but the token is named by its internal
`Debug` name rather than as `';'`. That rendering is a known bug.

### Exit codes

| Code | Meaning |
|---|---|
| 0 | The script ran to the end |
| 1 | A lex, parse or runtime error; the message is on stderr |
| 101 | The interpreter panicked — always a bug, please report it |

Exit 101 is reserved for panics so a crash can never be mistaken for a clean
error. A handful of inputs still reach it, all of them pathological:

```matlab
fprintf('%.65536f\n', 1);
```

```

thread '<unnamed>' (27164) panicked at src\builtins\core.rs:601:25:
Formatting argument out of range
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
```

(The process id in that message changes from run to run.)

A few others abort with 134 instead: a result size that only broadcasting or
`matmul` could produce (`ones(1e5,1) + ones(1,1e5)`), and about 96,000 levels
of nesting. These are the cycle-01d and 01e work.

## Differences from MATLAB

Everything here is verified against this binary. It is not the complete bug
list — `docs/ARCHITECTURE.md` has the "Known deviations" and "Known bugs"
tables with the cycle each is scheduled to — but it is the set you are most
likely to hit.

### Display

An exact zero prints as `0.0000` inside a fixed-point row, empties print as
`[]`, integer columns are too narrow at 1000 and above, there is no common
scale factor, `det` of an integer matrix prints as an integer, and scalar
display never switches to exponential form.

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

    0.0000    1.5000

b =

     []

c =

   1000   2000

d =

 1000.5000 2000.5000

e =

    -2

f =

 1234.5000

g =

    0.0010

h =

   10000000000

```

MATLAB gives `0 1.5000`, `1x0 empty double row vector`, wider integer columns,
`1.0e+03 *` with a scaled row, `-2.0000`, `1.2345e+03`, `1.0000e-03` and
`1.0000e+10`.

A single non-finite element forces the whole row into four-decimal format,
where MATLAB keeps integer columns:

```matlab
disp([1 2 NaN])
disp(NaN)
x = 0:0.1:0.3;
a = x(end) == 0.3
b = size(0:Inf)
```

```
    1.0000    2.0000       NaN
       NaN
a =

     0

b =

     1     0

```

MATLAB gives `     1     2   NaN`, `   NaN`, `1` for `a`, and refuses `0:Inf`
outright. The `a` line is the colon operator missing its end point: MATLAB
computes the second half of the range from the right-hand end so it lands
exactly on `b`. `linspace` has the same defect.

### Logical values

Comparisons return doubles, not logicals, which breaks masking in the most
dangerous possible way — quietly:

```matlab
disp(3 > 1)
x = [5 6 7];
y = x(x > 0)
x(x > 0) = 0
```

```
     1
y =

     5     5     5

x =

     0     6     7

```

MATLAB gives `   1`, `5 6 7` and `0 0 0`. The mask `[1 1 1]` is being read as
the index list "element 1, element 1, element 1". **Do not use logical
indexing yet.** Use `find` instead. Cycle 02 makes comparisons logical and
cycle 03 implements the indexing.

`NaN` converts silently to true, and `&&`/`||` accept non-scalars, where
MATLAB errors in both cases:

```matlab
if NaN, disp('NaN is true'), end
a = NaN & 1
b = ~NaN
c = [1 1] && 1
```

```
NaN is true
a =

     1

b =

     0

c =

     1

```

### Numerics

`matmul` skips a multiply when one factor is zero, so `Inf` and `NaN` are
swallowed; and results that should be complex are a silent `NaN` rather than
an error:

```matlab
a = [Inf 0] * [0; 1]
b = sqrt(-4)
c = log(-1)
d = (-8)^(1/3)
```

```
a =

     0

b =

       NaN

c =

       NaN

d =

       NaN

```

MATLAB gives `NaN`, `0 + 2.0000i`, `0 + 3.1416i` and `1.0000 + 1.7321i`.
Until complex numbers arrive in cycle 10, a clean error would be the honest
answer; these `NaN`s are not.

`trace([])` is `-0`, and `%d` saturates at 2^63:

```matlab
fprintf('%.4f\n', trace([]));
fprintf('%d\n', 1e30);
```

```
-0.0000
9223372036854775807
```

### Char class

Indexed assignment into a char silently makes it numeric, and the
rearrangement builtins lose the class. `+'a'` keeps it where MATLAB gives 97.

```matlab
s = 'abc';
s(1) = 'X'
t = fliplr('abc')
u = +'a'
```

```
s =

    88    98    99

t =

    99    98    97

u =

    'a'

```

MATLAB gives `'Xbc'`, `'cba'` and `97`. Plain concatenation of two chars does
keep the class (`['ab' 'cd']` is `'abcd'`); it is concatenation with a numeric
empty that loses it (`s = []; s = [s 'abc']` gives `97 98 99`).

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
all fail; use `clear('x')`. Chained ranges are rejected (`1:2:3:4` is
`unexpected Colon`, MATLAB reads it as `(1:2:3):4`). Hex literals (`0x1F`) are
rejected. A UTF-8 BOM at the start of a file is rejected, which a Windows
editor or PowerShell can easily produce.

A `break` or `continue` outside a loop ends the script silently with exit 0,
where MATLAB errors:

```matlab
disp(1)
break
disp(2)
```

```
     1
```

`2` never prints and the exit code is 0.

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

Empty-result shapes differ in several places: `find([])` and `diag([])` are
0x1 where MATLAB gives 0x0, `size('')` is `1 0` where MATLAB gives `0 0`, and
`disp([])` prints `[]` where MATLAB prints nothing.

```matlab
a = size(find([]))
b = size(diag([]))
c = size('')
disp([])
d = num2str([1 2; 3 4])
```

```
a =

     0     1

b =

     0     1

c =

     1     0

     []
d =

    '1  3  2  4'

```

`num2str` of a matrix gives one row in column-major order; MATLAB gives a
2x4 char array of `'1  2'` and `'3  4'`.

### Error text

Three message texts differ from current MATLAB, deliberately unresolved for
now: `Undefined function or variable 'x'.` is
`Unrecognized function or variable 'x'.` in R2020a and later; the size-mismatch
message names the operator and the sizes where MATLAB says only
`Arrays have incompatible sizes for this operation.`; and the `x(0)` text ends
`must be positive integers or logical values.` in MATLAB.

### REPL

REPL diagnostics go to stdout rather than stderr, so a piped session cannot
separate them from output. An incomplete block at end of input is discarded
silently.

## Not yet

None of the following exist. `docs/ROADMAP.md` gives the order.

| Missing | Arrives in |
|---|---|
| Logical indexing `x(x > 0)`, and the logical class | 02, 03 |
| Element deletion `x(i) = []` | 03 |
| Multiple assignment `[r, c] = size(A)` | 03 |
| Trailing singleton subscripts `A(2, 1, 1)` | 03 |
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
| Matrix `sort`, `[s, i] = sort(...)` | 09 |
| Complex numbers, `1i`, `real`, `imag`, `abs` of a complex | 10 |
| N-D arrays `zeros(2, 3, 4)` | not scheduled |
| `fprintf(fid, ...)`, `nbytes = fprintf(...)`, string functions | 11 |
| Regular expressions, file I/O, `save` / `load` | 11 |
| Plotting | 12 |
| `exit` in a script, `exit(code)`, `format`, `help`, `eval` | 13 |
| Integer classes and `single` | not scheduled |

Hitting one of these gives a parse error or
`Undefined function or variable`, never a wrong answer:

```matlab
f = @(x) x + 1
```

```
Error: Line 1: unexpected character '@'
```

```matlab
plot(1:10)
```

```
Error: Line 1: Undefined function or variable 'plot'.
```
