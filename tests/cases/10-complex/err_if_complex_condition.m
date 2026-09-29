% covers: 12 - (Scope, a kernel that ignores im) a complex if condition is a clean error, exit 1, never a test of its real part alone
% The text is SplatCrab's own, recorded in the spec's Design notes. The real
% part of 1i is zero, so a test that dropped the imaginary part would skip.
if 1i, disp(1), end
