% covers: 8 - cellfun with 'UniformOutput', false returns a cell of the results
r = cellfun(@(x) x * 2, {1, 2}, 'UniformOutput', false); disp(class(r)); disp(r{2})
