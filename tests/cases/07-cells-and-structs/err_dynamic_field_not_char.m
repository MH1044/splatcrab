% covers: 6 - (Scope, dynamic fields) a dynamic field name that is a number, not a char row, is a clean error, exit 1
% SplatCrab's own text (the spec's Messages), pinned here.
% Line 5 prints first, so the error comes at run time; the Error: prefix
% counts this covers line as line 1.
s.a = 1; disp(s.a)
y = s.(5)
