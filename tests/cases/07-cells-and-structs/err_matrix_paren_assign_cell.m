% covers: 2 - (Scope, cell write) assigning a cell into a double matrix with parentheses is a clean error, exit 1
% MATLAB's wording as recalled, not confirmed (the spec's Messages).
% Line 5 prints first, so the error comes at run time; the Error: prefix
% counts this covers line as line 1.
x = [1 2]; disp(numel(x))
x(2) = {1}
