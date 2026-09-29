% covers: 15 - fzero of x^2 + 1, which has no sign change to find, ends at its bracket search cap with a clean error, exit 1, never 101, 134 or a hang
% Item 15 lets the implementation choose exit 0 or 1 and record it; the
% Design notes record 1, a clean error, as Scope's caps ask.
% The text is SplatCrab's own, recorded in the spec's Design notes.
% NOTE: MATLAB warns and returns what it has; SplatCrab ends in a clean
% error (Known deviations, "Numerics, cycle 09").
x = fzero(@(x) x^2 + 1, 0);
