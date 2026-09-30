% covers: 15 - the builtins that take an N-D array keep its shape: double, logical and char convert it, fprintf and sprintf read every element in column-major order, deal copies it, and class and the class queries answer for it
d = double(true(1, 1, 2));
disp(class(d))
disp(size(d))
disp(d(:)')
g = logical(zeros(1, 1, 2));
disp(class(g))
disp(size(g))
disp(g(:)')
h = char(zeros(1, 1, 2) + 65);
disp(class(h))
disp(size(h))
disp(h(:)')
fprintf('%d ', reshape(1:8, 2, 2, 2)); fprintf('\n')
disp(sprintf('%d,', reshape(1:4, 1, 2, 2)))
disp(class(zeros(2, 2, 2)))
[p, q] = deal(zeros(1, 1, 2));
disp(size(q))
disp([isa(d, 'double'), islogical(g), ischar(h), isnumeric(d), iscell(d), isstruct(d), isreal(d)])
