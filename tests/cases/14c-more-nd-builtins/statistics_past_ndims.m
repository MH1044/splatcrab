% covers: 4 - along a dimension past ndims median and mode return the argument, mode's count 1 for each element and 0 for a NaN, and std and var return zeros the size of the argument, a NaN or Inf element included
A = reshape(1:24, 2, 3, 4);
disp(isequal(median(A, 4), A))
disp(isequal(mode(A, 4), A))
[M, F] = mode(A, 4);
disp(isequal(F, ones(2, 3, 4)))
disp(isequal(std(A, 0, 4), zeros(2, 3, 4)))
disp(isequal(var(A, 1, 4), zeros(2, 3, 4)))
disp(isequal(var(A, 0, 7), zeros(2, 3, 4)))
disp(var(NaN, 0, 3))
disp(std([NaN Inf], 0, 3))
disp(var([1 Inf; NaN 4], 1, 3))
disp(std([1 2; 3 4], 0, 3))
[M, F] = mode([1 NaN], 3);
disp(M)
disp(F)
disp(median([1 NaN], 3))
