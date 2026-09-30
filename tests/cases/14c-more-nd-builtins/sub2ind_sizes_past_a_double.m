% covers: 10 - sub2ind of sizes whose strides pass what a double holds: a subscript of 1 adds nothing to the index, scalar or array, so the index is that of the other subscripts, never NaN
disp(sub2ind([1e300 1e300 2], 1, 1, 1))
disp(sub2ind([1e300 1e300 2], 2, 1, 1))
disp(sub2ind([1e300 1e300 2], [1 2], 1, [1 1]))
c = num2cell(ones(1, 2000));
disp(sub2ind(2 * ones(1, 2000), c{:}))
c{2} = 2;
disp(sub2ind(2 * ones(1, 2000), c{:}))
