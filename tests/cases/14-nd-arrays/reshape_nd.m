% covers: 4 - reshape to and from N-D with several sizes, a size vector and one [] placeholder anywhere, keeping column-major order and dropping trailing sizes of 1
A = reshape(1:24, 2, 3, 4);
disp(size(A))
B = reshape(A, 6, []);
disp(size(B))
disp(B(:, 2)')
disp(size(reshape(A, [4 6])))
disp(isequal(reshape(A, [4 6]), reshape(1:24, 4, 6)))
disp(size(reshape(1:6, 1, 2, 3)))
disp(size(reshape(1:24, 2, [], 4)))
disp(size(reshape(1:6, 2, 3, 1)))
disp(size(reshape(A, [2 2 3 2])))
