% covers: 5 - c', c.' and transpose(c) of a cell array interchange the row and column index of every element, each element unchanged, never transposed or conjugated; a struct array alike, through ' and .'
% The values are the spec's (S5 and Scope, cells and structs transpose).
% d{3} is still the row [3 4], and z{1} keeps its imaginary part 2.
c = {1, 'ab', [3 4]}; d = c'; disp(size(d)); disp(d{2}); disp(d{3})
disp(isequal(c', c.'))
disp(size(transpose(c)))
g = {1, 2, 3; 4, 5, 6}'; disp(size(g)); disp(g{1, 2}); disp(g{3, 1})
z = {1+2i}'; disp(imag(z{1}))
s(1).a = 1; s(2).a = 2; t = s'; disp(size(t)); disp(t(2).a)
t = s.'; disp(size(t)); disp(t(2).a)
