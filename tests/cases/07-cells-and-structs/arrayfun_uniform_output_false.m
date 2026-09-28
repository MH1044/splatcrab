% covers: 14 - arrayfun with 'UniformOutput', false returns a cell, so each result may be an array
r = arrayfun(@(x) x * [1 1], 1:2, 'UniformOutput', false); disp(class(r)); disp(r{2})
