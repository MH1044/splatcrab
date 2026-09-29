% covers: 8 - (Scope, ode45 outputs) t is a column from tspan(1) to tspan(end), and y has a row per time and a column per component
% t(end) is checked to 1e-12 rather than with ==, so the case does not pin
% how the last step is clipped to the end of tspan.
[t, y] = ode45(@(t, y) [y(2); -y(1)], [0 1], [0; 1]);
disp(t(1) == 0); disp(abs(t(end) - 1) < 1e-12); disp(size(t, 2)); disp(size(y, 2)); disp(numel(t) == size(y, 1))
