% covers: 6 - flip reverses the elements along the first dimension whose size is not 1, or along a given dimension: an N-D array along dimension 3, a row, a column, a matrix's columns and rows, a 1x1x3 array along its third dimension, a dimension past ndims returning the array, and the class of a char or logical array kept
A = reshape(1:24, 2, 3, 4);
F = flip(A, 3);
disp(size(F))
disp(F(:, :, 1))
disp(F(:, :, 4))
disp(isequal(flip(A), flipud(A)))
disp(isequal(flip(A, 2), fliplr(A)))
disp(flip(1:3))
disp(flip((1:3)'))
disp(flip([1 2; 3 4]))
disp(flip([1 2; 3 4], 2))
disp(isequal(flip(A, 5), A))
x = flip(cat(3, 1, 2, 3));
disp(size(x))
disp(x(:)')
disp(flip('abc'))
disp(class(flip('abc')))
disp(class(flip(A > 12, 3)))
disp(size(flip(zeros(2, 0, 3), 2)))
