% covers: 3 - diff of an N-D array: along dimension 3; along the first dimension whose size is not 1 by default, each later round along the first dimension whose size is not 1 of what the round before left; n rounds along a given dimension leaving max(size - n, 0) elements there; a dimension past ndims giving an empty there, a result of exactly 2^20 dimensions made; n of 0 returning the array; the result a double
A = reshape(1:24, 2, 3, 4);
D = diff(A, 1, 3);
disp(size(D))
disp(D(:, :, 1))
disp(all(D(:) == 6))
D = diff(A);
disp(size(D))
disp(all(D(:) == 1))
disp(size(diff(A, 2)))
disp(isequal(diff(A, 2), zeros(1, 2, 4)))
disp(size(diff(A, 3)))
disp(size(diff(A, 4)))
disp(isequal(diff(A, 3, 3), zeros(2, 3)))
disp(size(diff(A, 4, 3)))
disp(size(diff(A, 5, 3)))
disp(size(diff(ones(2, 3), 1, 3)))
disp(size(diff(A, 1, 4)))
disp(ndims(diff(1:3, 1, 2^20)))
disp(isequal(diff(A, 0, 7), A))
disp(class(diff(A > 12, 1, 3)))
