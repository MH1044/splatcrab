% covers: 10 - acos outside [-1, 1] is complex, and cos of it gives the argument back
% Replaces 01d's err_complex_acos. The first line is the spec's, acos(2); the
% second is the refusal's own input, acos(-2), checked the same way.
disp(abs(cos(acos(2)) - 2) < 1e-12)
disp(abs(cos(acos(-2)) + 2) < 1e-12)
