% covers: 5 - reading an N-D array with any number of subscripts: a full subscript, a page, a row and a column across pages, two subscripts folding the trailing dimensions into the last, end in each position, a linear subscript, A(:), a logical mask and trailing subscripts of 1
A = reshape(1:24, 2, 3, 4);
disp(A(2, 3, 4))
disp(A(:, :, 2))
disp(size(A(1, :, :)))
disp(size(A(:, 1, :)))
v = A(1, 2, :);
disp(v(:)')
disp(A(2, 7))
disp(A(end))
disp(A(1, end))
disp(A(1, end, 1))
disp(A(end, end, end))
disp(A(A > 20)')
disp(A(1, 1, 1, 1))
disp(A(17))
disp(size(A(:)))
disp(isequal(A(:), (1:24)'))
