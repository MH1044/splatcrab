% covers: 13 - N-D behaviours pinned: cell2mat joins N-D elements by the bracket rule, max and min of an N-D logical keep the class logical, max and min over 'all' give the value and its linear index, and a shape of exactly 2^20 dimensions is made
disp(size(cell2mat({ones(2, 2, 2), ones(2, 1, 2)})))
disp(class(max(true(2, 2, 2), [], 3)))
disp(class(min(true(2, 2, 2))))
A = reshape(1:24, 2, 3, 4);
[m, i] = max(A, [], 'all');
disp([m i])
[m, i] = min(-A, [], 'all');
disp([m i])
X = A;
X(2, 1, 3) = 100;
[m, i] = max(X, [], 'all');
disp([m i])
disp(ndims(zeros([ones(1, 2^20 - 1) 2])))
