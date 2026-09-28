% covers: 15 - sort puts NaN last and leaves the finite values correctly ordered
% NOTE: bullet 15 spells this disp(sort([5 4 NaN 2 1])), which MATLAB prints as
% NOTE: "     1     2     4     5   NaN". SplatCrab used to pad a row containing
% NOTE: a NaN to the four-decimal width, so the ordering is checked with fprintf
% NOTE: here and only the all-finite row through disp. Cycle 01e fixed the
% NOTE: format and that disp line now prints MATLAB's text exactly; restoring it
% NOTE: rewrites a .out outside 01e's own directory, so cycle 02 does it. See
% NOTE: the Known bugs table of docs/ARCHITECTURE.md.
fprintf('%g %g %g %g %g\n', sort([5 4 NaN 2 1]));
fprintf('%g %g %g\n', sort([NaN 1 NaN]));
fprintf('%g %g %g\n', sort([NaN -Inf Inf]));
disp(sort([5 4 2 1]))
