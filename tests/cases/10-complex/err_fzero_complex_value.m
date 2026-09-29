% covers: 12 - (Scope, the solvers of cycle 09) fzero of a function with complex values is a clean error, exit 1, never the root of its real part
% The real parts, -1 and 1, change sign across the interval, so a solver that
% dropped the imaginary part would return 0. The text is SplatCrab's own,
% recorded in the spec's Design notes.
x = fzero(@(x) x + 1i, [-1 1]);
