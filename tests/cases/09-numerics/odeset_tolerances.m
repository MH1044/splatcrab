% covers: 8 - (Scope, odeset) an odeset struct of RelTol and AbsTol reaches ode45: more steps, and an error far inside the default one
% Both lines are self-checks. The step count grows roughly as tol^(-1/5),
% so RelTol 1e-8 against the default 1e-3 takes several times as many steps.
f = @(t, y) -y; opts = odeset('RelTol', 1e-8, 'AbsTol', 1e-10);
[t1, y1] = ode45(f, [0 1], 1); [t2, y2] = ode45(f, [0 1], 1, opts);
disp(numel(t2) > numel(t1)); disp(abs(y2(end) - exp(-1)) < 1e-6)
