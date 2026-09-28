% covers: 14 - recursion through a handle meets the same limit as a direct call: a clean error, exit 1, never an abort (134)
% Lines count this covers line as line 1. The error leaves every frame, so
% the Error: prefix names this script's own statement, line 5; the trace
% that follows it is not asserted.
r = viah(1)
function r = viah(n)
h = @viah;
r = h(n + 1);
end
