% covers: 11 - ind2sub gives as many subscript arrays as outputs, each a double the size of the indices: its page's examples, fewer outputs than sizes folding the sizes from the last output on, one output giving the indices themselves, more outputs than sizes reading 1s after them, the last dimension unbounded, a size given as a column, and matrix and N-D indices
[row, col] = ind2sub([3 3], [3 4 5 6]);
disp(row)
disp(col)
[I1, I2, I3] = ind2sub([2 2 2], [3 4 5 6]);
disp(I1)
disp(I2)
disp(I3)
[row, col, page] = ind2sub([3 4 2], 14);
disp([row col page])
[row, col] = ind2sub([2 2 2], 1:8);
disp(row)
disp(col)
row = ind2sub([2 2 2], 1:8);
disp(row)
[row, col] = ind2sub([3 1], [9 11 13 14]);
disp(row)
disp(col)
[r, c, p] = ind2sub([2 3], 5);
disp([r c p])
[r, c, p] = ind2sub([2 3], 7);
disp([r c p])
[r, c] = ind2sub([2 3], [1 2; 3 4]);
disp(size(r))
disp(r)
disp(c)
[r, c] = ind2sub([2 3], reshape(1:8, 2, 2, 2));
disp(size(r))
disp(size(c))
disp(r(:)')
disp(c(:)')
[r, c] = ind2sub([2; 3], 4);
disp([r c])
disp(class(r))
