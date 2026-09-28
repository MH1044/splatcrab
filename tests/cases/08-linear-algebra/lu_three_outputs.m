% covers: 2 - [L, U, P] = lu(A) pivots by rows so that P*A = L*U, with the multiplier 1/3 below the diagonal
% Partial pivoting takes row 2 first, since |3| > |1|, so L(2, 1) is 1/3.
% The residual comparison is a logical, which disp prints four wide.
A = [1 2; 3 4]; [L, U, P] = lu(A); fprintf('%.4f\n', L(2, 1)); disp(norm(P * A - L * U) < 1e-12)
