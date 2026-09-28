% covers: 12 - (Scope, brace write) a brace assignment into a variable holding a double is a clean error, exit 1
% MATLAB's wording as recalled, not confirmed (the spec's Messages): the brace form of item 12's dot-assignment text.
% Line 5 prints first, so the error comes at run time; the Error: prefix
% counts this covers line as line 1.
x = 1; disp(x)
x{1} = 2
