% covers: 2 - find of an N-D array: the linear indices as a column, the first or last n of them, a 0x1 column when no element is nonzero, a column for a 1x3x2 and a 1x1x3 array; with two outputs the column index running over every dimension past the first, so X(row(i), col(i)) is the ith nonzero element, and with three the values in the argument's class, every output a column
A = reshape(1:24, 2, 3, 4);
k = find(A > 22);
disp(size(k))
disp(k')
[r, c] = find(A == 14);
disp([r c])
[r, c, v] = find(A .* (A > 22));
disp([r c v])
[r, c, v] = find(A > 20);
disp(size(r))
disp(size(c))
disp([r c]')
disp(class(v))
disp(size(v))
disp(A(r(3), c(3)))
disp(find(A > 20, 1))
disp(find(A, 2, 'last')')
disp(size(find(A, 2, 'last')))
disp(size(find(zeros(2, 2, 2))))
disp(size(find(ones(1, 3, 2))))
disp(size(find(ones(1, 1, 3))))
