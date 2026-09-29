% covers: 8 - (Scope, ode45) ode45 of a function returning two values for one component is a clean error, exit 1
% The text is SplatCrab's own, recorded in the spec's Design notes.
[t, y] = ode45(@(t, y) [1; 2], [0 1], 1);
