% covers: 3 - isequal of structs: equal when of the same size with the same field names in any order and every field of every element equal; a field of another value, a field missing, or an element of a struct array changed makes them unequal; a struct never equals a cell
% The values are the spec's (S3 and Scope, isequal of cells and structs).
s.a = 1; s.b = 'x'; t.b = 'x'; t.a = 1;
disp(isequal(s, t))
t.b = 'y'; disp(isequal(s, t))
u.a = 1; disp(isequal(s, u))
p(1).a = 1; p(2).a = 2; q = p;
disp(isequal(p, q))
q(2).a = 3; disp(isequal(p, q))
disp(isequal(s, {1}))
