% covers: 10 - command syntax passes each word as a char argument, so clear x y removes both
% The Scope's own example. Each read is caught, so one script shows both.
x = 1; y = 2; z = 3;
clear x y
disp(z)
try, x, catch e, disp(e.message), end
try, y, catch e, disp(e.message), end
