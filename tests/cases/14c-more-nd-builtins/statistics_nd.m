% covers: 4 - median, std, var and mode of an N-D array along dimension 3 and along the first dimension whose size is not 1, the size there becoming 1: median's middle values, std and var normalised by N - 1 or by N, mode's smallest of a tie with its count the size of the result, a NaN in a slice making median NaN and ignored by mode, and a slice with no elements giving NaN and a count of 0
A = reshape(1:24, 2, 3, 4);
disp(median(A, 3))
M = median(A);
disp(size(M))
disp(2 * M(:)')
S = std(A, 0, 3);
disp(size(S))
fprintf('%.4f\n', S(1, 1))
disp(all(abs(S(:) - S(1, 1)) < 1e-12))
fprintf('%.4f\n', std(A(1, 1, :), 1))
disp(var(A, 1, 3))
disp(var(A, 0, 3))
disp(size(var(A)))
disp(isequal(2 * var(A), ones(1, 3, 4)))
X = cat(3, [1 2], [1 3], [2 3]);
[M, F] = mode(X, 3);
disp(M)
disp(F)
[M, F] = mode(A, 3);
disp(M)
disp(F)
disp(isequal(mode(A), A(1, :, :)))
disp(median(cat(3, 1, NaN, 3), 3))
[M, F] = mode(cat(3, 1, NaN, NaN), 3);
disp([M F])
M = median(zeros(2, 0, 3), 2);
disp(size(M))
disp(M(:)')
[M, F] = mode(zeros(2, 0, 3), 2);
disp(size(F))
disp(F(:)')
disp(all(isnan(M(:))))
disp(size(mode(zeros(2, 0, 3))))
S = std(zeros(2, 0, 3), 0, 2);
disp(size(S))
disp(all(isnan(S(:))))
