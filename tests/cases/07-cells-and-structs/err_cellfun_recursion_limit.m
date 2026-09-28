% covers: 8 - (Scope, call_nested) recursion through cellfun, handed a handle to the function that called it, meets the recursion limit: a clean error, exit 1, never a stack overflow (134)
% cellfun calls through Interp::call_nested, cycle 05's rule, as arrayfun
% has since cycle 06. The text is the limit's, as cycle 06's
% err_arrayfun_recursion_limit pins it; the Error: prefix names this
% script's own statement, line 6, counting this covers line as line 1.
r = viac(1);
function r = viac(n)
r = cellfun(@viac, {n + 1});
end
