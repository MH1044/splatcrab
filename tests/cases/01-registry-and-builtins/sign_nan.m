% covers: 23 - sign(NaN) is NaN
% NOTE: bullet 23 spells this disp(sign(NaN)), which MATLAB prints as "   NaN".
% NOTE: SplatCrab used to pad a non-finite value to the four-decimal width, so
% NOTE: the scalar NaN is checked with fprintf here. Cycle 01e fixed the format
% NOTE: and that disp line now prints MATLAB's text exactly; restoring it
% NOTE: rewrites a .out outside 01e's own directory, so cycle 02 does it. See
% NOTE: the Known bugs table of docs/ARCHITECTURE.md.
fprintf('%g\n', sign(NaN));
disp(sign([-3 0 5]))
fprintf('%g %g %g\n', sign([-Inf 0 Inf]));
disp(isnan(sign(NaN)))
