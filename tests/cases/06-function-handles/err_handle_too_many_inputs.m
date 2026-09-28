% covers: 10 - calling an anonymous function with more arguments than it has parameters is the too-many-inputs error
% Lines count this covers line as line 1. The handle is made and called on
% line 4, so the Error: prefix names line 4 whichever of the two it takes.
f = @(x) x; f(1, 2)
