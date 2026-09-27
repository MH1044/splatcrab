# 09 — Numerics

## Goal

`polyfit polyval roots(real) interp1 trapz cumtrapz diff fzero fminsearch integral ode45 odeset conv deconv filter std var median mode factorial nchoosek primes isprime gcd lcm unique ismember setdiff intersect union logspace meshgrid histc`

After this module a user can do things that were not possible before; every
bullet in Scope must be demonstrable by at least one acceptance test below.

## Scope

- `polyfit polyval roots(real) interp1 trapz cumtrapz diff fzero fminsearch integral ode45 odeset conv deconv filter std var median mode factorial nchoosek primes isprime gcd lcm unique ismember setdiff intersect union logspace meshgrid histc`

## Out of scope

- Anything not listed in Scope. Features named in a later module's Goal belong
  to that module; if this cycle needs one of them, shrink this spec instead of
  borrowing from the next.
- Anything returning complex results; those must error clearly until cycle 10.
- Stiff solvers such as ode15s.

## Design notes

Filled in as decisions are made. Start from the
"Key designs to preserve" section of docs/ARCHITECTURE.md and record here:
which files and types changed, which invariants were preserved (column-major
storage, 1-based to 0-based conversion at the index boundary, the `end` stack,
name resolution order, the output sink), and any deviation from MATLAB that
was accepted deliberately.

## Acceptance tests

Each numbered item becomes at least one golden case in
`tests/cases/09-numerics/`. Expected output is written by hand from MATLAB
semantics wherever feasible; the `.err` substring is always chosen by hand.
Every new error message needs an `err_*` case.

1. `p = polyfit([1 2 3], [2 4 6], 1); fprintf('%.4f %.4f\n', abs(p)); disp(polyval([1 0 -1], 3))` → `2.0000 0.0000\n     8`
2. `r = sort(roots([1 -3 2])); fprintf('%.4f %.4f\n', r)` → `1.0000 2.0000`
3. `disp(interp1([1 2 3], [10 20 30], 2.5)); disp(interp1([1 2 3], [10 20 30], 0.5)); disp(interp1([1 2 3], [10 20 30], 2.4, 'nearest'))` → `    25\n   NaN\n    20`
4. `disp(trapz([1 2 3])); disp(trapz([0 1 2], [0 1 4])); fprintf('%.1f ', cumtrapz([1 2 3])); fprintf('\n'); disp(diff([1 4 9 16])); disp(diff([1 4 9 16], 2)); disp(diff([1 2; 4 8]))` → `     4\n     3\n0.0 1.5 4.0 \n     3     5     7\n     2     2\n     3     6`
5. `fprintf('%.6f\n', fzero(@(x) x^2 - 2, 1)); fprintf('%.6f\n', fzero(@(x) cos(x) - x, [0 1]))` → `1.414214\n0.739085`
6. `x = fminsearch(@(x) (x(1) - 1)^2 + (x(2) - 2)^2, [0 0]); fprintf('%.3f %.3f\n', x)` → `1.000 2.000`
7. `fprintf('%.4f\n', integral(@(x) x.^2, 0, 1)); fprintf('%.4f\n', integral(@(x) exp(-x.^2), -Inf, Inf))` → `0.3333\n1.7725`
8. `[t, y] = ode45(@(t, y) -y, [0 1], 1); fprintf('%.3f\n', y(end)); [t, y] = ode45(@(t, y) [y(2); -y(1)], [0 pi], [0; 1]); fprintf('%.3f\n', y(end, 2))` → `0.368\n-1.000`
9. `disp(conv([1 2], [1 3])); fprintf('%.2f ', filter(1, [1 -0.5], [1 0 0])); fprintf('\n'); disp(filter([1 1] / 2, 1, [2 4 6]))` → `     1     5     6\n1.00 0.50 0.25 \n     1     3     5`
10. `fprintf('%.4f %.4f\n', std([1 2 3 4]), var([1 2 3 4])); disp(median([3 1 2])); disp(mode([1 2 2 3])); disp(factorial(5)); disp(nchoosek(5, 2)); disp(gcd(12, 18)); disp(lcm(4, 6)); disp(primes(20))` → `1.2910 1.6667\n     2\n     2\n   120\n    10\n     6\n    12\n     2     3     5     7    11    13    17    19`
11. `disp(unique([3 1 2 1])); [tf, loc] = ismember([2 5], [1 2 3]); disp(tf); disp(loc); disp(logspace(0, 2, 3)); [X, Y] = meshgrid(1:2, 1:3); disp(size(X))` → `     1     2     3\n   1   0\n     2     0\n     1    10   100\n     3     2`

## Status

Planned
