% covers: 7 - (Scope, struct arrays) a field assignment to a whole struct array of two elements is a clean error, exit 1
% MATLAB's wording as recalled, not confirmed (the spec's Messages).
% Line 5 prints first, so the error comes at run time; the Error: prefix
% counts this covers line as line 1.
p(2).a = 1; disp(numel(p))
p.a = 3
