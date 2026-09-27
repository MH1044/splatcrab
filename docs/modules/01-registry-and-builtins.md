# 01 — Registry and builtins

## Goal

Move the 78 builtins out of one 409-line `match` in `src/interp.rs` and into a
registry of ordinary functions, with `nargout` in the signature so that later
cycles can add multiple return values without touching every builtin again.
Behaviour does not change: the 21 baseline golden cases must pass untouched.

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
- `clear('a')` must clear only `a`, not the whole workspace.
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

Recommended approach, to be confirmed or corrected during implementation.

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

## Acceptance tests

Each becomes at least one golden case in `tests/cases/01-registry-and-builtins/`,
except where marked as a unit test.

1. The 21 existing `00-baseline` cases pass with no change to any `.out`. This
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

## Status

Planned
