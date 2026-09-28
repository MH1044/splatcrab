% covers: Scope, recursion limit - recursion through feval meets the same limit as a direct
% call: a clean error, exit 1, never a stack overflow
r = h(1);
function r = h(n)
r = feval('h', n + 1);
end
