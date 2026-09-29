% covers: 8 - ode45 of y' = -y from y(0) = 1 reaches exp(-1) at t = 1 (a self-check, within the default tolerances)
[t, y] = ode45(@(t, y) -y, [0 1], 1); disp(abs(y(end) - exp(-1)) < 1e-3)
