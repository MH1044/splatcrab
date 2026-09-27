% covers: 18 - a dimension beyond the array is a singleton, so the input is returned unchanged
A = [1 2; 3 4];
disp(sum(A, 3))
disp(prod(A, 3))
disp(mean(A, 4))
disp(sum([1 2 3], 5))
disp(size([1 2 3], 3))
