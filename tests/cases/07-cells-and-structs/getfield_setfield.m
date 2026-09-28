% covers: 4 - (Scope, builtins) getfield reads a field; setfield returns a copy with the field set, or added, and leaves its argument unchanged
% isfield returns a logical, which disp prints four wide.
s.a = 1; disp(getfield(s, 'a')); t = setfield(s, 'a', 5); disp(t.a); disp(s.a); u = setfield(s, 'b', 2); disp(isfield(u, 'b'))
