% covers: 10 - roots of x^2 + 1 is the complex pair +-i: its imaginary parts, sorted
% Replaces 09's err_roots_complex, the refusal of the same input.
r = sort(imag(roots([1 0 1]))); fprintf('%.4f %.4f\n', r)
