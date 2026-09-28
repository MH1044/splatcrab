% covers: 7 - (Scope, struct arrays) assigning a struct with other fields into an element of a struct array is a clean error, exit 1
% MATLAB's wording as recalled, not confirmed (the spec's Messages).
% Line 5 prints first, so the error comes at run time; the Error: prefix
% counts this covers line as line 1.
s.a = 1; t.b = 2; disp(s.a)
s(2) = t
