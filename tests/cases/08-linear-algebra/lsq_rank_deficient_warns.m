% covers: 1 - (Scope, least squares) a rank-deficient non-square system warns with SplatCrab's own text, returns the basic solution and exits 0
% The three rows of [1 1] make a rank-one 3-by-2 system. Which column the
% pivoted QR keeps is a tie the spec does not fix, so the case checks the
% basic solution by its properties: one entry is exactly zero and the two
% sum to 2, the least-squares value of x1 + x2 for b = [1; 2; 3].
x = [1 1; 1 1; 1 1] \ [1; 2; 3];
disp(size(x)); disp(any(x == 0)); disp(abs(sum(x) - 2) < 1e-12)
