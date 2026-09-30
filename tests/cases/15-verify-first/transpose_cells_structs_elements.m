% covers: 5 - (Scope, cells and structs transpose) a transposed cell keeps every element as it was, a handle, a column and a nested cell included, and an empty cell gives the empty of the interchanged size; a transposed struct array keeps its fields in their order, element (i, j) moving to (j, i)
% The values follow the Scope rule: element (i, j) of the result is
% element (j, i) of the argument, unchanged.
k = {@sin, [1; 2]; {1, 2}, 'xy'}; m = k';
disp(size(m))
disp(func2str(m{1, 1}))
disp(size(m{1, 2}))
disp(size(m{2, 1}))
disp(m{2, 2})
disp(size(cell(1, 0)'))
disp(size({}'))
r(1).b = 1; r(1).a = 2; r(2).b = 3; r(2).a = 4; w = r';
f = fieldnames(w); disp(f{1}); disp(f{2})
disp(size(w)); disp(w(2).a)
q(2, 3).a = 6; v = q.';
disp(size(v)); disp(v(3, 2).a); disp(isempty(v(2, 1).a))
