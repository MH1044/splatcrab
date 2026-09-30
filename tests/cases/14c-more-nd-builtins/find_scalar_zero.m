% covers: 2 - find of a scalar zero, double or logical, with or without a count, is the 0x0 empty for every output, while every other 2-D answer is unchanged: a zero row gives 1x0, the 0x0 empty 0x0, a zero matrix 0x1 and a nonzero scalar 1x1
disp(size(find(0)))
disp(size(find(false)))
[r, c] = find(0);
disp(size(r))
disp(size(c))
[r, c, v] = find(false);
disp(size(r))
disp(size(c))
disp(size(v))
disp(size(find(0, 1)))
disp(size(find(false, 2, 'last')))
[r, c] = find(0, 1);
disp([size(r) size(c)])
disp(size(find(zeros(1, 3))))
disp(size(find([])))
disp(size(find(zeros(2, 3))))
disp(size(find(1)))
