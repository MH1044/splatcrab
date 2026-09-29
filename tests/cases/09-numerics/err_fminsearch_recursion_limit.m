% covers: 15 - (Scope, call_nested) recursion through fminsearch, whose function calls the function that called it, meets the recursion limit: a clean error, exit 1, never a stack overflow (134)
% Each level is a user call and an anonymous call, so the limit of 500 is
% met long before the nesting limit. The text is the limit's, as cycle 06's
% err_arrayfun_recursion_limit pins it; the Error: prefix names this
% script's own statement, line 6, counting this covers line as line 1.
r = viam(1);
function r = viam(n)
r = fminsearch(@(x) x^2 + viam(n + 1), 1);
end
