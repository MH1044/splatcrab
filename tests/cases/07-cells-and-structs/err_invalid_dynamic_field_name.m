% covers: 6 - (Scope, dynamic fields) a dynamic field name that is not an identifier is a clean error, exit 1
% MATLAB's wording as recalled, not confirmed (the spec's Messages).
% Line 5 prints first, so the error comes at run time; the Error: prefix
% counts this covers line as line 1.
s.a = 1; disp(s.a)
s.('a b') = 2
