% covers: 5 - fzero from a scalar start searches for a sign change and converges to sqrt(2)
% sqrt(2) = 1.41421356... is 6e-8 clear of the 1.4142135 boundary of %.6f.
fprintf('%.6f\n', fzero(@(x) x^2 - 2, 1));
