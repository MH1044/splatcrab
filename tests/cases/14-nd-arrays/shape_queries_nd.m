% covers: 3 - size folds the remaining dimensions into its last output, returns 1 for extra outputs and for a dimension past ndims; ndims is 3 for a 2x3x4 and 2 for every 2-D value, a cell, a struct, a handle and an exception included; numel, length, isempty, isvector and isscalar read every dimension
[r, c] = size(zeros(2, 3, 4));
disp([r c])
[a, b, c, d] = size(zeros(2, 3, 4));
disp([a b c d])
disp(size(zeros(2, 3, 4), 2))
disp(size(zeros(2, 3, 4), 3))
disp(size(zeros(2, 3, 4), 5))
disp(ndims(zeros(2, 3, 4)))
disp(ndims(5))
disp(ndims({}))
disp(ndims('ab'))
disp(ndims(struct('a', 1)))
disp(ndims(@sin))
try
  error('boom');
catch e
end
disp(ndims(e))
disp(numel(zeros(2, 3, 4)))
disp(length(zeros(2, 5, 3)))
disp(length(zeros(2, 0, 3)))
disp(isempty(zeros(2, 0, 3)))
disp(isempty(zeros(2, 3, 4)))
disp(isvector(zeros(1, 1, 3)))
disp(isscalar(zeros(1, 1, 3)))
