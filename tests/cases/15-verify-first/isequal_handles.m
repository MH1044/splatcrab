% covers: 1 - isequal of function handles: two named handles are equal when they name the same function, str2func and a local function of the script included; two anonymous functions made separately are unequal whatever their text, and a copy equals its original; a named handle never equals an anonymous one, and a handle equals nothing but a handle
% The values are the spec's (S3, S4 and Scope, isequal of function
% handles): the first three lines are S4's own examples.
fun1 = @sin; fun2 = @sin; disp(isequal(fun1, fun2))
A = 5; h1 = @(x) A * x.^2; h2 = @(x) A * x.^2;
disp(isequal(h1, h2))
h2 = h1; disp(isequal(h1, h2))
disp(isequal(@sin, @cos))
disp(isequal(str2func('sin'), @sin))
disp(isequal(@sin, @(x) sin(x)))
disp(isequal(@sin, 1))
c = {h1}; disp(isequal(c{1}, h1))
disp(isequal(h1, h1, h1))
disp(isequal(@loc, @loc))
function y = loc(x)
y = x;
end
