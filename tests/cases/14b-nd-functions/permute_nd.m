% covers: 7 - permute makes dimension i of the result dimension dimorder(i) of the argument, a dimension past ndims being of size 1: an N-D array's elements moved to their new subscripts and moved back, a matrix's plain transpose, a matrix moved into the second and third dimensions, an order past ndims giving the array, an empty array, and the class and complex storage kept
A = reshape(1:24, 2, 3, 4);
P = permute(A, [3 1 2]);
disp(size(P))
disp(P(4, 2, 3))
disp(P(1, 1, 2))
disp(P(:, :, 1))
disp(P(:)')
disp(isequal(permute(P, [2 3 1]), A))
disp(size(permute(ones(2, 3), [2 1])))
M = reshape(1:6, 2, 3);
disp(isequal(permute(M, [2 1]), M.'))
disp(size(permute(ones(2, 3), [3 1 2])))
Q = permute(M, [3 1 2]);
disp(Q(:)')
disp(isequal(permute(A, [1 2 3 4]), A))
disp(size(permute(zeros(2, 0, 3), [3 1 2])))
c = permute('ab', [2 1])
disp(class(c))
L = permute(A > 12, [3 1 2]);
disp(class(L))
disp(isequal(L, P > 12))
W = [1+2i, 3; 4, 5i];
disp(isequal(permute(W, [2 1]), W.'))
disp(isequal(permute(W, [2 1]), W'))
Z = permute(A * 1i, [2 3 1]);
disp(isreal(Z))
disp(size(Z))
disp(isequal(imag(Z), permute(A, [2 3 1])))
