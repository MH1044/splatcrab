% covers: 6 - [U, S, V] = svd(A) (one-sided Jacobi) gives A back as U*S*V'
[U, S, V] = svd([1 2; 3 4]); fprintf('%.4f\n', norm(U * S * V' - [1 2; 3 4]));
