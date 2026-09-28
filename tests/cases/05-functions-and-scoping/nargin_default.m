% covers: 3 - nargin counts the arguments actually passed, so a missing one can take a default
disp(f(1)); disp(f(1, 2))
function r = f(a, b)
if nargin < 2, b = 10; end
r = a + b;
end
