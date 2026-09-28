% covers: 6 - struct(name, value, ...) builds a struct, a field chain indexes into a field, an assignment through a missing field creates the nested struct, and s.(n) reads the field a char names
s = struct('x', 5, 'y', [1 2]); disp(s.y(2)); s.inner.v = 3; s.inner.v = s.inner.v + 1; disp(s.inner.v); n = 'x'; disp(s.(n))
