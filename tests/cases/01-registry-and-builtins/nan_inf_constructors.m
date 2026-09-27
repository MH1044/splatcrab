% covers: 25 - NaN(2) and Inf(2, 3) fill a matrix instead of discarding the arguments
% NOTE: MATLAB's disp(NaN(2)) is two rows of "   NaN   NaN". SplatCrab pads a
% NOTE: non-finite value to the four-decimal width until the display rework in
% NOTE: cycle 02, so the fill values are checked with fprintf here.
disp(size(NaN(2)))
disp(all(all(isnan(NaN(2)))))
fprintf('%g %g %g %g\n', NaN(2));
disp(size(Inf(2, 3)))
disp(all(all(isinf(Inf(2, 3)))))
disp(size(NaN(2, 3)))
disp(size(NaN))
disp(size(Inf))
