% covers: 2 - (Scope, cell write) assigning a double into a cell with parentheses is a clean error, exit 1: c(k) = v needs a cell v
% MATLAB's wording as recalled, not confirmed (the spec's Messages).
% Line 5 prints first, so the error comes at run time; the Error: prefix
% counts this covers line as line 1.
c = {1}; disp(numel(c))
c(2) = 5
