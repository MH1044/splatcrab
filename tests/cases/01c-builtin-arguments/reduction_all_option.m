% covers: 19 - 'all' reduces over every element in sum, prod, mean, any, all, and max(A, [], 'all') and min
% NOTE: any and all return a logical in MATLAB, and disp of a logical is four
% NOTE: characters wide ("   1"). SplatCrab has no logical class until cycle 02
% NOTE: and prints six wide. Adding 0 makes the value a double in both, so the
% NOTE: expected "     1" below is MATLAB's own output, and stays so after 02.
A = [1 2; 3 4];
disp(sum(A, 'all'))
disp(prod(A, 'all'))
fprintf('%.4f\n', mean(A, 'all'));
disp(0 + any([0 0; 0 NaN], 'all'))
disp(0 + all(A, 'all'))
disp(max(A, [], 'all'))
disp(min(A, [], 'all'))
fprintf('%.4f\n', mean([1 2; 3 5], 'all'));
disp(0 + any([0 0; 0 3], 'all'))
disp(0 + all([1 0; 1 1], 'all'))
disp(max([1 NaN; 3 2], [], 'all'))
disp(min([4 NaN; 3 2], [], 'all'))
disp(sum(zeros(0, 3), 'all'))
disp(prod(zeros(0, 3), 'all'))
disp(size(sum(ones(2, 3), 'all')))
