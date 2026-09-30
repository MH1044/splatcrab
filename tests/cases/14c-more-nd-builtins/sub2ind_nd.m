% covers: 10 - sub2ind gives, as a double the size of the subscripts that are not scalars, the linear index of each set of subscripts: its page's examples, a scalar beside a column, fewer subscripts than sizes folding the trailing sizes into the last, sizes past the end of sz being 1, one subscript alone, a size given as a column, matrix and N-D subscripts, indices that read an N-D array as its subscripts do, and empty subscripts
disp(sub2ind([3 3], [1 2 3 1], [2 2 2 3]))
disp(sub2ind([2 2 2], [1 2 1 2], [2 2 1 1], [1 1 2 2]))
disp(sub2ind([3 4 2], 2, 1, 2))
disp(sub2ind([3 4], [1; 2], 3))
disp(sub2ind([2 3 4], 2, 12))
disp(sub2ind([2 3], 1, 2, 1))
disp(sub2ind([2 3], 6))
disp(sub2ind([2; 3], 2, 3))
disp(sub2ind([2 3], [1 2; 1 2], [1 1; 3 3]))
A = reshape(1:24, 2, 3, 4);
i = sub2ind(size(A), [1 2], [3 1], [4 2]);
disp(A(i))
k = sub2ind([2 3 4], ones(1, 1, 2), 3 * ones(1, 1, 2), reshape([2 4], 1, 1, 2));
disp(size(k))
disp(k(:)')
disp(size(sub2ind([2 3], 1, 1)))
disp(class(sub2ind([2 3], 1, 1)))
disp(size(sub2ind([2 3], zeros(1, 0), zeros(1, 0))))
