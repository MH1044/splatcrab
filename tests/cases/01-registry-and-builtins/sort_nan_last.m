% covers: 15 - sort puts NaN last and leaves the finite values correctly ordered
% NOTE: MATLAB's disp of [1 2 4 5 NaN] is "     1     2     4     5   NaN",
% NOTE: because a NaN does not stop MATLAB using the integer column format.
% NOTE: SplatCrab pads a row containing NaN to the four-decimal width until the
% NOTE: display rework in cycle 02, so the ordering is checked with fprintf here
% NOTE: and only the all-finite row is checked through disp.
fprintf('%g %g %g %g %g\n', sort([5 4 NaN 2 1]));
fprintf('%g %g %g\n', sort([NaN 1 NaN]));
fprintf('%g %g %g\n', sort([NaN -Inf Inf]));
disp(sort([5 4 2 1]))
