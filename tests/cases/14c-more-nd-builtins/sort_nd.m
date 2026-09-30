% covers: 1 - sort of an N-D array along dimension 3 and along the first dimension whose size is not 1 by default, ascending and descending, the index output the size of the argument holding each element's position along the dimension, shuffled pages sorted back, and a char array keeping its class
A = reshape(1:24, 2, 3, 4);
B = sort(-A, 3);
disp(size(B))
disp(B(:, :, 1))
disp(B(:, :, 4))
[B, I] = sort(A, 3, 'descend');
disp(size(I))
disp(I(:, :, 1))
disp(I(:, :, 4))
disp(isequal(B(:, :, 1), A(:, :, 4)))
disp(isequal(sort(A(:, :, [3 1 4 2]), 3), A))
[B, I] = sort(A(:, :, [3 1 4 2]), 3);
disp(I(:, :, 1))
disp(size(sort(A)))
disp(isequal(sort(A), A))
[B, I] = sort(-A);
disp(isequal(B(1, :, :), -A(2, :, :)))
disp(isequal(I(1, :, :), 2 * ones(1, 3, 4)))
s = sort(cat(3, 'b', 'a'));
disp(class(s))
disp(s(:)')
