% covers: 8 - matrix norm (2, 'fro', 1 and inf) and cond, closing the vectors-only norm deviation
% A'*A = [10 14; 14 20] has eigenvalues (30 +- sqrt(884))/2, so norm(A), the
% largest singular value, is 5.46499 and cond(A), the ratio of the two, is
% (30 + sqrt(884))/4 = 14.93303. 'fro' is sqrt(30) = 5.47723; the 1-norm is
% the largest column sum, 6, and the inf-norm the largest row sum, 7.
A = [1 2; 3 4]; fprintf('%.4f %.4f %.4f %.4f %.4f\n', norm(A), norm(A, 'fro'), norm(A, 1), norm(A, inf), cond(A))
