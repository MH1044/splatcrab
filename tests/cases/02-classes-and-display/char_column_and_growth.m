% covers: 20 - s(:) of a char is a char column, contiguous growth keeps a char char, and every rearrangement and true/false form keeps its class
s = 'abc';
t = s(:);
disp(class(t))
disp(size(t))
disp(t)
u = 'ab';
u(3) = 'c';
disp(class(u))
disp(u)
u(end+1) = 'd';
disp(class(u))
disp(u)
disp(class(flipud('ab')))
disp(class(repmat('ab', 1, 2)))
disp(class(reshape('abcd', 2, 2)))
disp(class('ab'''))
disp(class(false))
disp(class(true(2)))
disp(class(true([1 2])))
disp(class(false(2, 3)))
