% covers: 10 - (Scope, deal) deal with three inputs and two outputs is a clean error, exit 1
% MATLAB's wording as recalled, not confirmed (the spec's Messages).
% Line 5 prints first, so the error comes at run time; the Error: prefix
% counts this covers line as line 1.
disp(1)
[a, b] = deal(1, 2, 3)
