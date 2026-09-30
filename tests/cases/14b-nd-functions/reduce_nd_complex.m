% covers: 1, 2 and 4 - the reductions keep today's complex rules on an N-D array: sum and mean along a dimension and cumsum along one keep the complex storage and every other size, their parts reduced as the real arrays they are
A = reshape(1:24, 2, 3, 4);
Z = A - 2i * A;
S = sum(Z, 3);
disp(size(S))
disp(isreal(S))
disp(real(S))
disp(imag(S))
M = mean(Z, 3);
disp(isreal(M))
disp(real(M))
disp(imag(M))
C = cumsum(Z, 3);
disp(size(C))
disp(isreal(C))
disp(isequal(real(C), cumsum(A, 3)))
disp(isequal(imag(C), -2 * cumsum(A, 3)))
T = sum(Z);
disp(size(T))
disp(isequal(imag(T), -2 * sum(A)))
