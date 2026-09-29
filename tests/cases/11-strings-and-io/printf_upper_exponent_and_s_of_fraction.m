% covers: 6 - (Scope, QA D16 a and b) %E and %G print an upper-case E, and %s of a non-integer uses %e
% sprintf('%s', pi) is the MATLAB sprintf page's own example, which the Known
% bugs row of docs/ARCHITECTURE.md records. Neither 12345.678 nor 1e-10 lands
% on a rounding tie.
disp(sprintf('%E', 12345.678))
disp(sprintf('%G', 1e-10))
disp(sprintf('%s', pi))
