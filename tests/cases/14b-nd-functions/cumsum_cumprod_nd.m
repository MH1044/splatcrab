% covers: 4 - cumsum and cumprod of an N-D array keep every size: cumsum along dimension 3 ends in the sum along it, cumsum along the first dimension by default, cumprod of a 1x1x3 runs along its third dimension, a dimension past ndims returns the array, and a dimension of size 0 keeps its size
A = reshape(1:24, 2, 3, 4);
C = cumsum(A, 3);
disp(size(C))
disp(C(:, :, 2))
disp(C(:, :, 4))
disp(isequal(C(:, :, 4), sum(A, 3)))
D = cumsum(A);
disp(size(D))
disp(D(:)')
P = cumprod(2 * ones(1, 1, 3))
disp(isequal(cumsum(A, 4), A))
disp(isequal(cumprod(A, 7), A))
disp(size(cumprod(zeros(2, 0, 3), 2)))
