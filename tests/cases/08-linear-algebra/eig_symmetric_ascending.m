% covers: 5 - eig of a symmetric matrix (Jacobi) returns its eigenvalues in ascending order
% [2 1; 1 2] has eigenvalues 1 and 3. They are printed unsorted, so the
% ascending order is part of what the case asserts.
e = eig([2 1; 1 2]); fprintf('%.4f %.4f\n', e);
