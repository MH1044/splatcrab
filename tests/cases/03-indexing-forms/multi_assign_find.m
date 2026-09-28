% covers: 9 - [r, c] = find(A) returns row and column subscripts in column-major order
% The nonzeros are A(2, 1) and A(1, 2), met in that order, so r is [2; 1] and
% c is [1; 2].
[r, c] = find([0 1; 1 0]);
disp([r c])
