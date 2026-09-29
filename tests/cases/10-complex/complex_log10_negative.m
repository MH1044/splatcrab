% covers: 10 - log10 of a negative number is complex, and 10 raised to it gives the number back
% Replaces 01d's err_complex_log10. The first line is the spec's, log10(-8);
% the second is the refusal's own input, log10(-10), checked the same way.
disp(abs(10^log10(-8) + 8) < 1e-12)
disp(abs(10^log10(-10) + 10) < 1e-12)
