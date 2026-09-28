% covers: 18 - dot indexing parses, and is a clean run-time error on a matrix
% A script is parsed whole before it runs, so the disp(x) output proves the
% error comes at run time, not from the parser.
x = [1 2];
disp(x)
x.a
