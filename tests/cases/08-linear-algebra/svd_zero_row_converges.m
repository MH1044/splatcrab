% covers: 6 - (Scope, every iterative method terminates) the one-sided Jacobi SVD of a matrix with a zero row converges instead of reaching its sweep cap
% A crash probe found this matrix reaching the cap. Its nonzero singular
% values are those of [1 2 3; 4 5 7], whose A*A' = [14 35; 35 90] has
% eigenvalues 52 +- sqrt(2669): 10.181472 and 0.581063 after the square root,
% clear of any %.4f tie. The third is 0, and rank agrees.
A = [1 2 3; 4 5 7; 0 0 0];
fprintf('%.4f %.4f %.4f\n', svd(A)); disp(rank(A))
