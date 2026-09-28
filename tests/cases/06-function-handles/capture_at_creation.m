% covers: 2 - an anonymous function captures a variable's value when it is created: the later a = 0 does not reach it
a = 10; f = @(x) x + a; a = 0; disp(f(1))
