% covers: 7 - (Scope, null) null(A) is an orthonormal basis of the null space, one column per missing rank
% [1 1] has rank 1, so its null space in R^2 is one line. The basis vector's
% sign is not specified, so the case checks it by its properties: A*N is zero
% and N has unit length.
A = [1 1]; N = null(A); disp(size(N)); disp(norm(A * N) < 1e-12); disp(abs(norm(N) - 1) < 1e-12)
