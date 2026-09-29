% covers: 12 - (Scope, the solvers of cycle 09) fzero from a complex starting point is a clean error, exit 1, never a search from its real part
% The text is SplatCrab's own, recorded in the spec's Design notes.
x = fzero(@(x) x - 1, 2i);
