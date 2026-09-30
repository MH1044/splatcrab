% covers: 1 - sum of an N-D array along dimension 3, along the first dimension by default with every other size kept, along the first dimension whose size is not 1, along dimension 2, along a dimension past ndims or of size 1 returning the array as a double, over 'all' elements, along a dimension of size 0 giving zeros of the kept sizes, and the empty 0x0 kept as today
A = reshape(1:24, 2, 3, 4);
T = sum(A, 3)
S = sum(A);
disp(size(S))
disp(S(:)')
disp(sum(ones(1, 1, 3)))
U = sum(ones(1, 3, 2));
disp(size(U))
disp(U(:)')
V = sum(A, 2);
disp(size(V))
disp(V(:)')
disp(size(sum(A, 5)))
disp(isequal(sum(A, 5), A))
disp(class(sum(A > 12, 5)))
disp(isequal(sum(A > 12, 5), double(A > 12)))
disp(isequal(sum(ones(2, 1, 3), 2), ones(2, 1, 3)))
disp(sum(A, 'all'))
Z = sum(zeros(2, 0, 3), 2);
disp(size(Z))
disp(Z(:)')
disp(sum([]))
