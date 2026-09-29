% covers: 15 - (Scope, invariant 6) fminsearch of x, which has no minimum, meets its iteration or evaluation cap: a clean error, exit 1, never a hang
% The simplex walks downhill for ever. The cap's text is cycle 08's
% no_convergence, as the spec's Design notes record.
% NOTE: MATLAB warns and returns what it has; SplatCrab ends in a clean
% error (Known deviations, "Numerics, cycle 09").
x = fminsearch(@(x) x, 1);
