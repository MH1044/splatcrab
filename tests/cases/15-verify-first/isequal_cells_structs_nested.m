% covers: 2, 3 - (Scope, isequal of cells and structs) elements compare by isequal's rules however cells and structs nest inside each other: arrays by size and value with the class not compared, a NaN equal to nothing; a cell never equals an array, even an empty one, and a struct never equals an array; a struct array against one of its elements, or a third argument that differs, is unequal
% The values follow the Scope rule. x and y are built apart, so the answer
% comes from their elements, not from one being a copy of the other.
x.c = {1, struct('d', 'e')}; y.c = {1, struct('d', 'e')};
disp(isequal(x, y))
y.c{2}.d = 'f'; disp(isequal(x, y))
disp(isequal({true}, {1}))
disp(isequal({[1 2]}, {[1; 2]}))
disp(isequal({1}, {1, 1}))
disp(isequal({}, []))
disp(isequal(struct('a', 1), 1))
disp(isequal(struct('a', NaN), struct('a', NaN)))
disp(isequal({1}, {1}, {2}))
p(1).a = 1; p(2).a = 2;
disp(isequal(p, p(1)))
