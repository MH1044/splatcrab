% covers: 8 - ode45 of the system y1' = y2, y2' = -y1 from [0; 1] gives y2 = cos(t), -1 at pi (a self-check)
[t, y] = ode45(@(t, y) [y(2); -y(1)], [0 pi], [0; 1]); disp(abs(y(end, 2) + 1) < 1e-3)
