% covers: 9 - (Scope, filter) filter whose denominator starts with 0 is refused with a clean error, exit 1
% The text is SplatCrab's own, recorded in the spec's Design notes.
y = filter(1, [0 1], [1 2 3]);
