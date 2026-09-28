% covers: 14 - (Scope, call_nested) a handle that calls a builtin that calls the handle stays bounded: f hands itself to feval, which calls it again, and the run ends in a clean error, exit 1, never a stack overflow (134)
% Every anonymous call counts against the recursion limit (Design notes,
% Calls), and each level adds one of them and a few nesting levels, so the
% limit of 500 is met long before the nesting limit of 10,000. Line 6
% counts this covers line as line 1.
f = @(g, n) feval(g, g, n + 1); f(f, 1)
