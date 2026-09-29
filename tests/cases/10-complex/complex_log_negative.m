% covers: 10 - log of a negative number is complex: log(-1) is 0 + pi*i
% Replaces 01d's err_complex_log, the refusal of the same input.
fprintf('%.4f %.4f\n', real(log(-1)), imag(log(-1)))
