% covers: 5 - eig of a nonsymmetric matrix with real eigenvalues (Hessenberg reduction and QR iteration)
% [4 1; 2 3] has trace 7 and determinant 10, so its eigenvalues are 2 and 5.
% The order an iteration finds them in is not specified, hence the sort, and
% %.4f absorbs the last bits.
fprintf('%.4f %.4f\n', sort(eig([4 1; 2 3])));
