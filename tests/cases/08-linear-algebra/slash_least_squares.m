% covers: 1 - (Scope, least-squares /) slash with a wide matrix is the least-squares solution of x*A = b, a row
% b / A is (A' \ b')', so this is backslash_least_squares transposed: the same
% c = 2/3 and m = 1/2, now as a 1-by-2 row.
x = [1 2 2] / [1 1 1; 1 2 3]; fprintf('%.4f %.4f\n', x); disp(size(x))
