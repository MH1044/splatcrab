% covers: 14 - (Scope, call_nested) recursion through feval of a handle meets the same limit: every level re-enters the interpreter from inside a builtin, and the run still ends in a clean error, exit 1, never a stack overflow (134)
% Lines count this covers line as line 1; the Error: prefix names this
% script's own statement, line 4.
r = viaf(1);
function r = viaf(n)
r = feval(@viaf, n + 1);
end
