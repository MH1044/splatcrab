% covers: 7 - (Scope, integral) integral of a function that returns NaN is a clean error, exit 1
% The text is SplatCrab's own, recorded in the spec's Design notes.
q = integral(@(x) NaN(size(x)), 0, 1);
