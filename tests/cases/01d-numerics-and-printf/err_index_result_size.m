% covers: 3 - a two-subscript index read sizes its result from the subscripts, not from the array
% A is 2x2. The result of A(i, j) is numel(i) by numel(j), so two 1e5-long
% index vectors ask for a 1e5-square result out of four elements. Exit 1,
% never the allocator's 134.
A = [1 2; 3 4];
A(ones(1, 1e5), ones(1, 1e5))
