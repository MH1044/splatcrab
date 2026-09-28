% covers: 14 - (Scope, call_nested) recursion through arrayfun, handed a handle to the function that called it, meets the same limit: a clean error, exit 1, never a stack overflow (134)
% Lines count this covers line as line 1; the Error: prefix names this
% script's own statement, line 4.
r = viaa(1);
function r = viaa(n)
r = arrayfun(@viaa, n + 1);
end
