% covers: 15 - (Scope, invariant 6) ode45 of y' = y^2 from y(0) = 1, whose solution 1/(1 - t) blows up at t = 1, meets its minimum step or step count cap: a clean error, exit 1, never a hang
% tspan runs to 2, past the pole, so no step size lets the solver finish.
% The text is SplatCrab's own, recorded in the spec's Design notes.
% The time it names is where the numerical solution blows up, a little
% before 1; only four digits of it are pinned.
% NOTE: MATLAB warns and returns what it has; SplatCrab ends in a clean
% error (Known deviations, "Numerics, cycle 09").
[t, y] = ode45(@(t, y) y^2, [0 2], 1);
