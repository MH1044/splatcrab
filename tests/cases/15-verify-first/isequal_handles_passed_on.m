% covers: 1 - (Scope, isequal of function handles) an anonymous handle equals the same handle passed on through a field, as an argument and back as an output; two anonymous functions made separately are unequal, with no captures and the same text; a handle equals nothing but a handle, in either order
% The values follow the Scope rule: anonymous handles compare by identity,
% a copy being the same handle, never by their text or their captures.
h = @(x) x + 1;
s.f = h; disp(isequal(s.f, h))
disp(same(h, h))
disp(isequal(pass(h), h))
disp(isequal(@(x) x, @(x) x))
g = @(x) x + 1; disp(isequal(g, h))
disp(isequal(h, h, g))
disp(isequal(1, @sin))
disp(isequal(@sin, 'sin'))
disp(isequal(@sin, {@sin}))
function r = same(a, b)
r = isequal(a, b);
end
function out = pass(h)
out = h;
end
