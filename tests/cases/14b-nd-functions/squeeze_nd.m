% covers: 6 - squeeze removes the dimensions of length 1, a result of fewer than two dimensions being a column; returns a row, a column, a scalar or an array with no dimension of length 1 as it is; keeps the elements in column-major order; and keeps the class and the complex storage
disp(size(squeeze(ones(1, 1, 3))))
disp(size(squeeze(zeros(2, 1, 3))))
disp(size(squeeze(zeros(1, 3, 1, 2))))
disp(size(squeeze(zeros(1, 1, 1, 4))))
disp(size(squeeze(ones(2, 3))))
disp(size(squeeze(ones(1, 5))))
disp(size(squeeze(ones(5, 1))))
disp(size(squeeze(zeros(1, 0, 3))))
disp(squeeze(7))
A = reshape(1:24, 2, 3, 4);
disp(isequal(squeeze(A), A))
S = squeeze(A(1, :, :))
v = squeeze(A(2, 3, :))
L = squeeze(true(1, 1, 3));
disp(class(L))
disp(size(L))
c = squeeze(reshape('abc', 1, 1, 3))
Z = squeeze(complex(ones(1, 1, 3), 0));
disp(isreal(Z))
disp(size(Z))
W = squeeze(reshape([0 2 3], 1, 1, 3) + 1i * reshape([1 0 0], 1, 1, 3));
disp(isreal(W))
disp(size(W))
disp(imag(W)')
