% covers: 3 - class() and the class-propagation table
% Arithmetic yields double (true + true), comparisons and ~ yield logical, and
% max of a logical keeps the class.
disp(class(5))
disp(class('a'))
disp(class(true))
disp(class(1:3 > 2))
disp(class(true + true))
disp(class(~1))
disp(class(max([true false])))
