% covers: 5 - (Scope, fzero) fzero of a function that returns a vector is a clean error, exit 1
% The text is SplatCrab's own, recorded in the spec's Design notes.
x = fzero(@(x) [x x], 1);
