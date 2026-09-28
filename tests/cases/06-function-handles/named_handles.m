% covers: 3 - @name makes a handle to a builtin, @sin, and to a function local to the script, @sq, each called through its variable
g = @sin; disp(g(0)); h = @sq; disp(h(3))
function r = sq(x)
r = x * x;
end
