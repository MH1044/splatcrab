% covers: 12 - unbounded recursion stops at the recursion limit of 500 with a clean error, exit 1, never an abort (134)
% The error names the script's line 4 and is followed by one stack line per
% frame that was running, each at its call on line 6; only the first is asserted.
inf_rec(1)
function r = inf_rec(n)
r = inf_rec(n + 1);
end
