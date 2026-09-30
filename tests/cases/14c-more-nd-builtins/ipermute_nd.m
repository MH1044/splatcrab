% covers: 8 - ipermute undoes permute, dimension dimorder(i) of the result being dimension i of the argument: a matrix and an N-D array moved back, an N-D array's elements moved to their subscripts, an order past the argument's ndims, and the class and complex storage kept
M = [1 2 3; 4 5 6; 7 8 9; 10 11 12];
B = permute(M, [2 1]);
disp(isequal(ipermute(B, [2 1]), M))
A = reshape(1:24, 2, 3, 4);
P = permute(A, [3 1 2]);
disp(isequal(ipermute(P, [3 1 2]), A))
disp(size(ipermute(ones(4, 2, 3), [3 1 2])))
R = ipermute(A, [3 1 2]);
disp(size(R))
disp(R(2, 3, 1))
disp(R(3, 4, 2))
disp(isequal(R, permute(A, [2 3 1])))
disp(size(ipermute(ones(2, 3), [3 1 2])))
disp(isequal(ipermute(A, [1 2 3 4]), A))
disp(isreal(ipermute(complex(ones(2, 3), 0), [2 1])))
disp(class(ipermute(A > 12, [3 1 2])))
c = ipermute('ab', [2 1]);
disp(class(c))
disp(size(c))
