% covers: 23 - sign(NaN) is NaN
% NOTE: MATLAB's disp(sign(NaN)) is "   NaN", because a NaN does not stop
% NOTE: MATLAB using the integer column format. SplatCrab pads a non-finite
% NOTE: value to the four-decimal width until the display rework in cycle 02,
% NOTE: so the scalar NaN is checked with fprintf here.
fprintf('%g\n', sign(NaN));
disp(sign([-3 0 5]))
fprintf('%g %g %g\n', sign([-Inf 0 Inf]));
disp(isnan(sign(NaN)))
