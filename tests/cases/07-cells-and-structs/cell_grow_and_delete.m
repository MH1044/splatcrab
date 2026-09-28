% covers: 2 - cell(1, 3) holds empties, a brace assignment past the end grows the cell, and deleting with parentheses removes an element
% isempty returns a logical, which disp prints four wide (cycle 02).
c = cell(1, 3); disp(isempty(c{1})); c{5} = 'x'; disp(numel(c)); c(2) = []; disp(numel(c))
