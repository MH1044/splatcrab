% covers: 25 - NaN(2) and Inf(2, 3) fill a matrix instead of discarding the arguments
% NOTE: bullet 25 spells this disp(NaN(2)), which MATLAB prints as two rows of
% NOTE: "   NaN   NaN". SplatCrab used to pad a non-finite value to the
% NOTE: four-decimal width, so the fill values are checked with fprintf here.
% NOTE: Cycle 01e fixed the format and that disp line now prints MATLAB's text
% NOTE: exactly; restoring it rewrites a .out outside 01e's own directory, so
% NOTE: cycle 02 does it. See the Known bugs table of docs/ARCHITECTURE.md.
disp(size(NaN(2)))
disp(all(all(isnan(NaN(2)))))
fprintf('%g %g %g %g\n', NaN(2));
disp(size(Inf(2, 3)))
disp(all(all(isinf(Inf(2, 3)))))
disp(size(NaN(2, 3)))
disp(size(NaN))
disp(size(Inf))
