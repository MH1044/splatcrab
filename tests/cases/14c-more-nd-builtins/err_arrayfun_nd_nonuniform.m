% covers: 12 - arrayfun with 'UniformOutput' false over an N-D argument is refused, since the cell array it would return would be N-D; exit 1
A = reshape(1:24, 2, 3, 4);
arrayfun(@(x) x, A, 'UniformOutput', false)
