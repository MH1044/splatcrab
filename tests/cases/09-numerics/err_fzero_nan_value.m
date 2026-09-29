% covers: 5 - (Scope, fzero) fzero of a function that returns NaN is a clean error, exit 1, never a hang
% The text is SplatCrab's own, recorded in the spec's Design notes.
x = fzero(@(x) NaN, 1);
