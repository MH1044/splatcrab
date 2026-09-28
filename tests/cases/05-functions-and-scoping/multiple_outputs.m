% covers: 2 - a function with two outputs fills both targets of a multiple assignment, and each is displayed
[s, p] = sp(2, 3)
function [s, p] = sp(a, b)
s = a + b; p = a * b;
end
