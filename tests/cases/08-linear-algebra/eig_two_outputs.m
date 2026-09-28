% covers: 5 - [V, D] = eig(A) returns the eigenvalues on the diagonal of D
[V, D] = eig([2 0; 0 3]); fprintf('%.4f %.4f\n', diag(D));
