% covers: 6 - fminsearch finds the minimum of a bowl centred at [1 2] (a self-check: the search stops at its tolerance)
x = fminsearch(@(x) (x(1) - 1)^2 + (x(2) - 2)^2, [0 0]); disp(norm(x - [1 2]) < 1e-3)
