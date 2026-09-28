% covers: 5 - (Scope, Jacobi symmetric eig) the eigenvectors of a symmetric matrix satisfy A*V = V*D and are orthonormal
% A is not diagonal, so the Jacobi sweep must rotate and accumulate V. Each
% vector's sign is not specified, so the case checks properties only.
A = [2 1; 1 2]; [V, D] = eig(A); disp(norm(A * V - V * D) < 1e-12); disp(norm(V' * V - eye(2)) < 1e-12)
