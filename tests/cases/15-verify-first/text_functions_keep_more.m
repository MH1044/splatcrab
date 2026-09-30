% covers: 9 - (Scope, text functions and char arrays of several rows) lower of a char array argument of several rows keeps its answer and shape; strtrim trims each element of a cell by its own rule, a char array of several rows beside a character vector; strtok's delimiters may be of any size; '' and a char with no rows are character vectors to strrep
% The values follow the Scope rule: strtrim of a char array of several
% rows removes a column only when every row has a blank there, and strtok
% splits at the first of its delimiter characters.
x = lower(['AB'; 'CD']); disp(x)
r = strtrim({[' ab'; ' cd'], ' e '}); disp(size(r{1})); disp(r{1}); disp(size(r{2}))
r = strtrim({['ab'; 'cd']}); disp(r{1})
[t, rest] = strtok('a-b', ['-'; '+']); disp(t); disp(rest)
disp(strrep('abc', 'a', ''))
disp(strrep('abc', 'b', char(zeros(0, 3))))
