% covers: Scope, Hessenberg+QR real eig - a 4x4 non-symmetric matrix reaches the Hessenberg
% reduction, the shifted QR iteration and the eigenvector back-substitution (a 2x2 is solved
% directly and reaches none of them). The spectrum is 1 2 3 4 by construction, A = T*D/T, and
% the vectors are checked by their residual rather than by their digits
T = [2 1 0 1; 1 3 1 0; 0 1 2 1; 1 0 1 4]; A = T * diag([1 2 3 4]) / T;
fprintf('%.4f ', sort(eig(A))); fprintf('\n');
[V, D] = eig(A); disp(norm(A * V - V * D) < 1e-12 * norm(A))
