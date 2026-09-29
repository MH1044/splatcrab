% covers: 3 - (Scope, interp1) interp1 with 'spline', a method this cycle does not provide, is refused with a clean error, exit 1
% The text is SplatCrab's own, recorded in the spec's Design notes.
v = interp1([1 2 3], [1 2 3], 1.5, 'spline');
