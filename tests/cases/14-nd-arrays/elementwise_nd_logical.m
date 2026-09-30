% covers: 8 - a comparison, &, | and ~ on an N-D array give a logical array of its shape
A = reshape(1:24, 2, 3, 4);
L = A > 12;
disp(class(L))
disp(size(L))
disp(isequal(L(:)', [false(1, 12), true(1, 12)]))
N = ~(A > 12);
disp(class(N))
disp(size(N))
disp(isequal(N(:)', [true(1, 12), false(1, 12)]))
B = reshape(1:8, 2, 2, 2);
M = B > 2 & B < 7;
disp(size(M))
disp(M(:)')
M = B < 2 | B > 7;
disp(M(:)')
M = B == 4;
disp(M(:)')
