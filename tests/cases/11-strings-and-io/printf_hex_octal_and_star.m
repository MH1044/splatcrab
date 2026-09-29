% covers: 6 - (Scope, QA D16 f) %x, %X and %o, and a * width or precision taken from the arguments
% ff and the four spaces before 3 are the Known bugs row's values, in
% docs/ARCHITECTURE.md. The brackets show the padding.
disp(sprintf('%x', 255))
disp(sprintf('%X', 255))
disp(sprintf('%o', 8))
disp(['[' sprintf('%*d', 5, 3) ']'])
disp(sprintf('%.*f', 2, pi))
