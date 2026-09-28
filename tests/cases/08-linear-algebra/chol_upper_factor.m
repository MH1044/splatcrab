% covers: 4 - chol returns the upper-triangular R with R'*R = A
% R = [2 1; 0 sqrt(2)], printed in column-major order: 2, 0, 1, 1.4142.
R = chol([4 2; 2 3]); fprintf('%.4f ', R); fprintf('\n');
