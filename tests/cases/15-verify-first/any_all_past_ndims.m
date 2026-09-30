% covers: 7 - any and all along a dimension past ndims give each element the answer they give it along a dimension of size 1, a logical array of the argument's shape: a NaN is ignored by any and nonzero to all; along a dimension of size 1 within ndims unchanged
% The values are the spec's (S7 and Scope, any and all past ndims), and the
% last two pairs show the same answers along a dimension of size 1.
disp(any(NaN, 3))
disp(all(NaN, 3))
disp(any([0 NaN 2], 3))
disp(all([0 NaN 2], 3))
disp(class(any(NaN, 3)))
disp(class(all([0 NaN 2], 3)))
disp(size(all(NaN(2, 3), 5)))
disp(all([1; NaN; 0], 4))
disp(any(NaN, 1))
disp(all(NaN, 1))
disp(any([0 NaN 2], 1))
disp(all([0 NaN 2], 1))
