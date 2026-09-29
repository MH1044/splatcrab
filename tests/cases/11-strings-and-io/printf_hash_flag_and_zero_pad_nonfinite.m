% covers: 6 - (Scope, QA D16 c and d) the # flag keeps the point of %.0f, and the 0 flag pads a non-finite value with spaces, not zeros
% Both values are the Known bugs row's, in docs/ARCHITECTURE.md. The brackets
% show the leading space.
disp(sprintf('%#.0f', 3))
disp(['[' sprintf('%05d', -Inf) ']'])
