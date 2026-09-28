% covers: 16 - a mask with no zeros is still a mask, not the positions 1 1 1 (QA D6)
% Takes the place of 02-classes-and-display/err_logical_index. Before cycle 02
% this printed 5 5 5 with no error.
x = [5 6 7];
disp(x(x > 0))
x(x > 0) = 0;
disp(x)
