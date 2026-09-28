% covers: 1 - an anonymous function of one, two or no parameters is called through the variable that holds it
f = @(x) x.^2; disp(f(4)); g = @(x, y) x + y; disp(g(1, 2)); z = @() 42; disp(z())
