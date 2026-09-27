% covers: vector indices, the shape of an indexed read, colon and end arithmetic
A = [1 2 3; 4 5 6];
v = [10 20 30];
disp(A(:)')
disp(v([2; 3]))
disp(A([1 2; 2 1]))
disp(v([1 2; 2 1]))
disp(A([1 2]))
disp(A([1; 2]))
fprintf('%d %d\n', size(A(:)));
fprintf('%d %d\n', size(A(:, :)));
fprintf('%d %d\n', size(A(1, :)));
fprintf('%d %d\n', size(A(:, 1)));
disp(A(1, end:-1:1))
fprintf('%d %d %d\n', A(end), A(end - 1), A(end, end));
