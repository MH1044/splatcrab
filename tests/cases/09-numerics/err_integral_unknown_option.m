% covers: 7 - (Scope, integral) integral with 'Waypoints', an option this cycle does not provide, is refused with a clean error, exit 1
% The text is SplatCrab's own, recorded in the spec's Design notes.
q = integral(@(x) x, 0, 1, 'Waypoints', 0.5);
