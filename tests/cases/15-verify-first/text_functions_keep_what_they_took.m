% covers: 9 - text functions keep what they took: upper of a char array argument of several rows keeps its answer and shape, strtrim trims a char array of several rows in a cell as it trims that array on its own, strcat of char arrays of several rows keeps its rule, and a char with no rows and '' are character vectors
% The values are the spec's (Scope, text functions and char arrays of
% several rows). strtrim removes a column only when every row has a blank
% there, so [' ab'; ' cd'] loses its first column and ['ab '; ' cd'] none.
x = upper(['ab'; 'cd']); disp(x)
r = strtrim({[' ab'; ' cd']}); disp(size(r{1})); disp(r{1})
r = strtrim({['ab '; ' cd']}); disp(size(r{1}))
disp(strcat(['a'; 'b'], ['x'; 'y']))
disp(numel(upper({char(zeros(0, 3))})))
x = upper({'ab', ''}); disp(x{1}); disp(isempty(x{2}))
