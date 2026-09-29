% covers: 10 - log2 of a negative number is complex, and 2 raised to it gives the number back
% Replaces 01d's err_complex_log2, the refusal of the same input, log2(-8).
disp(abs(2^log2(-8) + 8) < 1e-12)
