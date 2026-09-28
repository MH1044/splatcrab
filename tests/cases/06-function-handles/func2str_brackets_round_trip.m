% covers: 6 - func2str renders from the parse tree, so bracket elements stay apart: str2func of its text makes the same function again
% Only the strings the spec records are asserted, so this case checks the
% round trip, not the text: [x 1] must keep two elements, and so must
% [x -1], which becomes the one element x-1 if the spaces are simply dropped.
f = str2func(func2str(@(x) [x 1])); disp(f(5))
g = str2func(func2str(@(x) [x -1])); disp(g(5))
