% covers: 8 - (Scope, ode45) ode45 integrates backwards over a decreasing time span: y' = -y from y(1) = exp(-1) back to t = 0 ends near 1
% A self-check: the value is roundoff at ode45's default tolerances.
[t, y] = ode45(@(t, y) -y, [1 0], exp(-1)); disp(abs(y(end) - 1) < 1e-3); disp(t(end))
