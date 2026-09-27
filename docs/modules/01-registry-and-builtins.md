# 01 — Registry and builtins

## Goal

Move the 78 builtins out of one 409-line `match` in `src/interp.rs` and into a
registry of ordinary functions, with `nargout` in the signature so that later
cycles can add multiple return values without touching every builtin again.
Behaviour does not change: the 26 baseline golden cases must pass untouched.

```matlab
sum(1, 2, 3)     % Too many input arguments.
x = disp(3)      % prints 3, then: Too many output arguments.
t = tic; toc(t)  % elapsed seconds
```

## Scope

- A `src/builtins/` module split into `mod.rs`, `args.rs`, `core.rs`,
  `math.rs` and `linalg.rs`.
- `pub type BuiltinFn = fn(&mut Interp, &[Value], usize) -> R<Vec<Value>>`,
  where the `usize` is `nargout`. A `Registry` mapping a name to an `Entry`
  holding the function pointer and a one-line help string, built once in
  `Interp::new`.
- All 78 existing builtins migrated **unchanged in behaviour**. Names that
  currently share a match arm (`zeros|ones|eye|rand`, `sum|prod|mean|any|all`,
  `max|min`, `cumsum|cumprod`, `who|whos`) become thin wrappers over one
  shared function so the sharing survives.
- The return convention: an empty `Vec` means the builtin produced no value.
  That is legal as a statement and is "Too many output arguments." in an
  expression, replacing today's "'disp' does not return a value."
- Argument helpers in `args.rs`, replacing the closures currently defined
  inside `call_builtin`: `mat`, `scalar`, `dim`, `string`, `need`. They keep
  producing the existing MATLAB-style messages.
- Arity checks: "Too many input arguments." when a builtin is given more
  arguments than it accepts, and "Not enough input arguments." kept as it is.
- `tic` and `toc`, including `toc(t)` with a handle from `tic`.
- The interpreter runs on a thread with a 256 MB stack, so deep recursion
  raises a clean error rather than overflowing the native stack. Windows gives
  the main thread only 1 MB by default.

### Defect fixes, folded in deliberately

The adversarial pass over the baseline found these, all inside builtins. They
are fixed here because this cycle already reads and rewrites every builtin, and
doing them in a second pass would mean touching the same 78 functions twice.
None of them is covered by an existing golden case, which is precisely why they
survived, so the "baseline cases must not move" criterion below still holds.
Full descriptions are in "Known bugs" in `docs/ARCHITECTURE.md`.

- **Two panics, fixed first.** `num2str(Inf)` overflows an `i32`, and a size
  argument such as `zeros(1e10)` overflows when `rows * cols` is computed. Both
  abort the process and kill the REPL, breaking invariant 6. Every arithmetic
  path that turns user input into a size or a digit count must be checked.
- `sort` with any `NaN` present: the comparator maps an incomparable pair to
  `Equal`, which is not a total order, so even the finite elements come out
  misplaced. MATLAB sorts the finite values and puts `NaN` last.
- `clear('a')` must clear only `a`, not the whole workspace. Reading the
  argument is what makes a non-character argument an error: `clear(5)` used to
  clear the whole workspace and exit 0, and is now `Argument 1 to 'clear' must
  be a character vector.`
- `cumsum` and `cumprod` must honour their dimension argument.
- Reductions must reject dimension `0` and treat any dimension beyond the
  array's as a singleton, returning the input unchanged, rather than silently
  giving the dimension-2 answer. `size(A, 0)` must error.
- Reductions on an empty with an explicit dimension must keep MATLAB's empty
  shape instead of collapsing to a scalar.
- `printf`: honour the `+` and space flags, honour precision on integer
  conversions, switch `%d` to `%e` rather than `%g` for a non-integer, print
  `%s` of a number as its character, and expand a char argument to one
  argument per character.
- `sign(NaN)` must be `NaN`.
- A negative size is `0`, as in MATLAB, not an error.
- `NaN(2)` and `Inf(2,3)` must fill a matrix rather than discarding their
  arguments and returning a scalar.

## Out of scope

- Any change to a builtin's behaviour beyond the defect list above. This cycle
  moves code. If another builtin looks wrong, record it in "Known bugs" in
  `docs/ARCHITECTURE.md` and schedule it rather than fixing it here.
- The two entries marked "verify first" in that table, `mod` and `rem` with an
  infinite divisor and the loop variable after a zero-iteration `for`. They
  were reasoned about rather than checked against a real MATLAB. Confirm the
  real behaviour before encoding either in a test.
- `true(n)` and `false(n)` taking size arguments, which belong with the logical
  class in cycle 02.
- The error type, line numbers in messages, and the lexer bug fixes. Those are
  cycle 01b. Doing them together would mean two wide refactors in one cycle.
- Multiple return values at the call site. This cycle only puts `nargout` into
  the signature; `Stmt::MultiAssign` arrives in cycle 03.

## Design notes

Recommended approach, confirmed and extended during implementation. The
decisions taken during the build are in "As built" at the end of this section.

**Order matters.** Do the registry before the error type, not after. The
signature is written in terms of the `R<T>` alias, so when cycle 01b changes
`R` from `Result<T, String>` to `Result<T, MError>`, no builtin signature
changes textually.

**The borrow problem.** `self.builtins.get(name)` borrows `self` immutably
while the builtin wants `&mut Interp`. Because `BuiltinFn` is a plain `fn`
pointer and therefore `Copy`, copy it out of the map first:

```rust
let f = self.builtins.get(name).map(|e| e.f);
if let Some(f) = f { return f(self, &args, nargout); }
```

**Where `nargout` comes from** in this cycle: `Stmt::Expr` passes 0, `eval`
passes 1. Nothing else calls a builtin yet.

**Keep the migration mechanical and verifiable.** Move builtins in groups and
run `cargo test --test golden` after each group. Any golden diff means the
move changed behaviour, which is the one thing this cycle must not do.

**The help strings** are one line each. Nothing consumes them yet; `help`
arrives in cycle 13. Write them anyway while the context is fresh.

### As built

Everything above held. What follows is what the implementation had to decide.

**There were 79 builtins, not 78.** Counting the names in the old match gives
79; `README.md`, `docs/FEATURES.md`, `docs/ROADMAP.md` and
`docs/modules/00-baseline.md` were one short, and all four now say 79 for the
baseline. With `tic` and `toc`
the registry now holds 81, and that is the number the registry unit test
asserts.

**Order of work.** The two panics were fixed first, in the old `match`, and
each was confirmed with a script that has to exit non-zero: `num2str(Inf)`
before the fix aborted with "attempt to add with overflow" at exit 101, and
`zeros(1e10)` with "attempt to multiply with overflow". `cargo test --test
golden` was green on all 26 baseline cases at that point, before a single
builtin moved.

**The migration was one move, not a sequence of groups.** Taking the builtins
out in groups needs the old `match` and the new registry to coexist, with the
registry falling back to the match for whatever has not moved yet, and that
scaffolding is itself untested code on the hot path of every call. The move
went in one step instead, and the "behaviour did not change" claim is backed
by something stronger than a per-group golden run: a build of the pre-cycle
commit and the new binary were both run over a script that calls all 79
original builtins, and the two outputs were diffed. They differ on exactly one
line, `fprintf('%s', 66)`, which is the deliberate fix that prints `B` rather
than `66`. Everything else is byte-identical, stdout and stderr. No baseline
`.out` moved at any point.

**Where the panic guard lives.** `args::check_size(rows, cols)` is the only
sanctioned way to turn a user-supplied shape into an allocation length. It
rejects both the overflowing product and a product larger than `MAX_ELEMS`,
2^28 elements or 2 GiB of `f64`, so `zeros(1e5)` fails cleanly rather than
dying in the allocator. `interp.rs` calls it too, for growth on indexed
assignment, which is the one non-builtin path the "Known bugs" entry named.
`check_size` closes this family only where a **size** reaches a builtin. It
does not close invariant 6, and this cycle should not be read as having done
so. Two other paths in the same family remain, both pre-existing and both
recorded in "Known bugs":

- `range`, the `:` operator: `1:1e15` aborts in the allocator. It is an
  operator rather than a builtin, so it never reaches `check_size`. Scheduled
  to 01b.
- `printf` width and precision: `fprintf('%.65536f', 1)` panics because Rust's
  formatter holds precision in a `u16`, and `fprintf('%2147483647d', 1)` hangs
  building the pad. The scope bullet above says every path turning user input
  into "a size or a digit count" must be checked, and a format precision is
  exactly a digit count, so this one was in scope to *notice* even though the
  fix belongs with the printf rework. Scheduled to 11.

Because the family is still open, `src/main.rs` joins the interpreter thread
with `unwrap_or(101)` rather than `unwrap_or(1)`. A panic must remain
distinguishable from a clean error by exit code, or a golden case with an
`.err` file, which expects exit 1, could pass on a panic.

**Six helpers, not five.** `args.rs` has `mat`, `scalar`, `dim`, `string` and
`need` as specified, and three more that the defect fixes forced apart:
`at_most` for "Too many input arguments.", `size_arg` for a size (where a
negative is `0`) as distinct from `dim` for a dimension (where `0` is an
error), and `check_size`. The old single `dim` closure did both jobs, which is
exactly why `zeros(-1)` errored and `sum(A, 0)` did not.

**Registration tables are `#[rustfmt::skip]`.** rustfmt's `fn_call_width` of
60 turns each `add(...)` into five lines, which buries the one thing a reader
wants from these functions: the list of names. One line per builtin, skipped.

**Closures for the one-liners.** A non-capturing closure coerces to
`BuiltinFn`, so the twenty-odd element-wise functions are registered as
`|_, a, _| unary(a, "abs", f64::abs)` rather than as twenty near-identical
named functions. Anything with a body worth testing on its own is a named
function instead.

**What moved and what stayed.** `format_printf`, its `PArg` type and the
`num2str` helper moved into `builtins/core.rs`, since only builtins use them.
`fmt_e` and `fmt_g` stayed in `interp.rs`: `value.rs` calls `fmt_e` from the
display path, which is not builtin territory, and cycle 02 reworks display
anyway. `reduce` moved to `math.rs` and gained `scan`, the `cumsum`/`cumprod`
equivalent that honours its dimension argument.

**The file split.** `core.rs` holds constants, constructors, shape queries,
output, the workspace and timing; `math.rs` the reductions, element-wise maths
and predicates; `linalg.rs` the linear algebra plus rearrangement (`reshape`,
`repmat`, `fliplr`, `flipud`) and search and sort. Rearrangement went with
linear algebra rather than with the shape queries to keep `core.rs` from
taking half the library.

**`%s` takes a whole char argument.** Expanding a char array to one argument
per character, which is what `fprintf('[%d %d]', 'AB')` needs, would have
broken the baseline's `fprintf('...%s...', 'ab')` if `%s` then consumed a
single character. So each flattened character carries the index of the
argument it came from, and `%s` consumes the whole run. `%d` still takes one.

**`tic` and `toc` are the first readers of `nargout`.** `t = tic` returns a
handle and leaves the shared mark alone, so a nested pair cannot disturb an
outer one; bare `tic` sets the mark and returns nothing. `toc` returns the
elapsed seconds when asked for a value and prints "Elapsed time is N seconds."
when not. The clock is an `Instant` taken in `Interp::new`, and a handle is
nanoseconds since then, so a handle is an ordinary double and needs no new
value type.

**`main` returns an exit code.** Running on the 256 MB thread means `main` no
longer returns normally from the interpreter, so `run` hands back a code and
flushes `Interp.out` before it does. Without the flush, output not ending in a
newline would be lost to `process::exit`.

**Bullets 15, 23 and 25 are asserted by value, not by column layout.** The
`disp` output the three bullets name is MATLAB's, but SplatCrab pads any row
holding a `NaN` or an `Inf` to the four-decimal width, so `disp(sort([5 4 NaN
2 1]))` prints `    1.0000    2.0000    4.0000    5.0000       NaN`. That is a
display bug, not a builtin bug: it lives in `value.rs`, it is already visible
as `x6` in `00-baseline/display_formats`, and fixing it would move a baseline
`.out`, which this cycle's primary acceptance criterion forbids. The three
cases therefore check the values with `fprintf('%g')` and record the `disp`
form in `% NOTE` lines; the display itself is a new "Known bugs" row scheduled
to cycle 02, which says to restore the `disp` lines when it lands.

**`repmat`'s overflow message names the requested array.** The first guard
checked `m.rows * r` and `m.cols * c` separately, so `repmat([1 2], 1e10,
1e10)` reported `Requested 1x10000000000`, an intermediate rather than the
`10000000000x20000000000` array asked for. The factors are now multiplied with
saturation and `check_size` judges the shape itself. A side effect is that
`repmat` on an empty with a huge count is an empty array rather than an error,
which is what MATLAB does.

**Bullet 14 covers eight paths, not five.** The bullet names `zeros`, `ones`,
`rand`, `eye` and `reshape`. `repmat`, `linspace`, `diag` and indexed growth
run through the same `check_size` and have cases too, because the claim that
one guard covers every path is worth a test rather than a sentence. `diag` is
the case that needs the `MAX_ELEMS` cap rather than the overflow check: `diag`
of a 1e5 vector asks for 1e10 elements without overflowing anything.

**Ten new defects found and not fixed**, recorded in "Known bugs". Three stand
on their own: the uncapped range above; `max([], [], 1)`, which gives `0x0`
where MATLAB gives `1x0`, because the empty check runs before the dimension
argument is read; and `pi(2)`, `e(2)` and `eps(2)`, which MATLAB fills and the
new arity check now rejects, and which belong with `true(n)` in cycle 02.

The other seven are the rest of that last family, and they are the price of the
arity checks. Nothing in the old interpreter looked at an argument count, so
every call form whose extra argument it silently dropped is now "Too many input
arguments.": `sort(v, 'descend')`, `find(x, k)`, `norm(v, p)`, `diag(A, k)` and
`diag(v, k)`, `num2str(x, n)`, `round(x, n)`, and the three-size forms
`zeros(r, c, p)`, `ones(r, c, p)` and `rand(r, c, p)`. Each was run under a
build of the pre-cycle commit as well as this one. The old code answered every
time, and every answer was wrong: an ascending sort, every index rather than
the first `k`, the 2-norm, the main diagonal, the default precision, and a 2-D
array with the third size thrown away. Erroring is better than answering
wrongly and is squarely inside the Scope bullet on arity checks, so none of it
is reverted; instead each is a row in "Known bugs", scheduled to the cycle that
implements the argument it names. `eye(r, c, p)` is not in the list: MATLAB
rejects it too.

## Acceptance tests

Each becomes at least one golden case in `tests/cases/01-registry-and-builtins/`,
except where marked as a unit test.

1. The 26 existing `00-baseline` cases pass with no change to any `.out`. This
   is the primary acceptance criterion for the whole cycle.
2. `sum(1, 2, 3)` → err `Too many input arguments.`
3. `x = disp(3)` → prints `     3`, then err `Too many output arguments.`
4. `disp(1)` as a statement → prints `     1` and exits 0, proving an empty
   return is legal at statement level.
5. `t = tic; x = toc(t); disp(x >= 0)` → `     1`
6. `tic; disp(class(toc))`-style check that bare `toc` returns a value, once
   `class` exists; until then `t = tic; disp(toc(t) >= 0)` → `     1`
7. `zeros(2,3)`, `ones(2)`, `eye(3)` and `rand(2,2)` still produce the right
   shapes, proving the shared-arm wrappers dispatch correctly.
8. `sum([1 2; 3 4])`, `prod`, `mean`, `any`, `all` with and without a
   dimension argument, proving the other shared arm.
9. `max([3 9 2])`, `max([1 2],[3 0])`, `min` likewise.
10. A deeply nested expression, for instance 5000 nested parentheses, is
    either evaluated or rejected with a clean error, and does not crash the
    process. This is what the 256 MB stack buys.
11. Unit: the registry contains exactly the expected number of names, and
    every name resolves to a callable entry.
12. Unit: `args::scalar` on a non-scalar and `args::need` with too few
    arguments produce the existing message text.

Defect fixes, one case each. Every one of these must exit 0, not abort:

13. `disp(num2str(Inf)); disp(num2str(-Inf))` → `Inf` and `-Inf`, with no
    panic. `disp(num2str(NaN))` → `NaN`.
14. `zeros(1e10)` → a clean error mentioning the requested size, exit code 1,
    no panic. Same for `ones`, `rand`, `eye` and `reshape`.
15. `disp(sort([5 4 NaN 2 1]))` → `     1     2     4     5   NaN`
16. `a=1; b=2; clear('a'); disp(b); disp(exist_check)` where the last line is
    `a` → `     2` then err `Undefined function or variable 'a'.`
17. `disp(cumsum([1 2; 3 4], 2))` → `     1     3` and `     3     7`;
    `disp(cumprod([1 2; 3 4], 2))` → `     1     2` and `     3    12`;
    `disp(cumsum([1 2 3], 1))` → `     1     2     3`
18. `disp(sum([1 2; 3 4], 3))` → the input unchanged; `sum([1 2; 3 4], 0)` →
    err about a positive integer dimension; `size([1 2 3], 0)` → the same err.
19. `disp(size(sum([], 1)))` → `     1     0`
20. `fprintf('[%+d][% d][%.3d][%+f]\n', 5, 5, 5, 1.5)` →
    `[+5][ 5][005][+1.500000]`
21. `fprintf('[%d]\n', pi)` → `[3.141593e+00]`
22. `fprintf('[%s]\n', 65)` → `[A]`; `fprintf('[%d %d]\n', 'AB')` → `[65 66]`
23. `disp(sign(NaN))` → `   NaN`; `disp(sign([-3 0 5]))` → `    -1     0     1`
24. `disp(size(zeros(-1)))` → `     0     0`; `disp(size(zeros(2,-3)))` →
    `     2     0`
25. `disp(NaN(2))` → a 2x2 of `NaN`; `disp(size(Inf(2,3)))` → `     2     3`
26. `abs(1, 2)` → err `Too many input arguments.`, covering the whole class of
    silently-accepted extra arguments.
27. `a = 1; clear(5)` → err `Argument 1 to 'clear' must be a character vector.`,
    the error path that reading `clear`'s argument opens, where the old code
    cleared the workspace and exited 0.

## Status

Done (2026-09-27). 35 golden cases in
`tests/cases/01-registry-and-builtins/`, 37 new unit tests for 151 in all
(35 of them in `src/builtins/`), and the 26 `00-baseline` cases passing with
no `.out` touched.
