% covers: 10 - asin outside [-1, 1] is complex, and sin of it gives the argument back
% Replaces 01d's err_complex_asin, the refusal of the same input, asin(2).
disp(abs(sin(asin(2)) - 2) < 1e-12)
