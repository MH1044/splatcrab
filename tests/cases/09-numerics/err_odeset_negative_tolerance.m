% covers: 8 - (Scope, odeset) odeset with a negative RelTol is refused with a clean error, exit 1
% The text is SplatCrab's own, recorded in the spec's Design notes.
o = odeset('RelTol', -1);
