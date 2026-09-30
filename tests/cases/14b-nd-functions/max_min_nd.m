% covers: 3 - max and min of an N-D array along any dimension with the index along it as a second output, along the first dimension by default, against a scalar and against an array broadcast across every dimension, along a dimension past ndims giving the array and indices of 1, ignoring NaN along the first dimension whose size is not 1, and over 'all' elements
A = reshape(1:24, 2, 3, 4);
[m, i] = max(A, [], 3)
[M, I] = min(A, [], 2);
disp(size(M))
disp(M(:)')
disp(size(I))
disp(I(:)')
X = max(A);
disp(size(X))
disp(X(:)')
G = max(A, 12);
disp(size(G))
disp(G(:)')
Q = min(A, reshape([3 30 20 10], 1, 1, 4));
disp(size(Q))
disp(Q(:)')
[m5, i5] = max(A, [], 5);
disp(isequal(m5, A))
disp(size(i5))
disp(isequal(i5, ones(2, 3, 4)))
disp(max(reshape([1 NaN 3 NaN], 1, 1, 4)))
disp(max(A, [], 'all'))
disp(min(A, [], 'all'))
