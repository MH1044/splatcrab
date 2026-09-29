% covers: 15 - a solver calling a solver: fzero's function is itself an fzero, nested through call_nested; the root is 0 (a self-check, since the raw value is roundoff)
disp(abs(fzero(@(x) fzero(@(y) y - x, 0), 1)) < 1e-6)
