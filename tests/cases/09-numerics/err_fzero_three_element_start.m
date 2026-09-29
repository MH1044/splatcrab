% covers: 5 - (Scope, fzero) fzero from a three-element start is refused with a clean error, exit 1
% The text is SplatCrab's own, recorded in the spec's Design notes.
x = fzero(@(x) x, [1 2 3]);
