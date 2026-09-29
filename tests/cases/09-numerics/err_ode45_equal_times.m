% covers: 8 - (Scope, ode45) ode45 over a time span of two equal times is refused with a clean error, exit 1
% The text is SplatCrab's own, recorded in the spec's Design notes.
[t, y] = ode45(@(t, y) -y, [0 0], 1);
