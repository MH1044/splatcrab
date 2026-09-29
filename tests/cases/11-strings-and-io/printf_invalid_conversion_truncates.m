% covers: 6 - (Scope, QA D16 g) an invalid conversion or a trailing % ends the output there: the text before it is printed, the rest discarded, and no error
% MATLAB "prints all text up to the invalid operator ... and discards the
% rest", as the Known bugs row in docs/ARCHITECTURE.md quotes it;
% sprintf('abc%q', 1) is that row's example.
disp(sprintf('abc%q', 1))
disp(sprintf('abc%'))
