% covers: MATLAB precedence for unary minus, power, transpose, comparison and range
fprintf('%g %g\n', -2^2, (-2)^2);
fprintf('%g %g\n', 2^-1, 2^3^2);
fprintf('%g\n', -2.^2);
fprintf('%g %g\n', 1 < 2 < 3, 3 > 2 > 1);
fprintf('%g %g\n', ~0 + 1, 2 + 3 * 4);
fprintf('%g %g\n', 2 * 3^2, max(1:3 + 1));
disp((1:3) == 1)
disp(-[1 2]')
