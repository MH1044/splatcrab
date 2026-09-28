% covers: 8 - an anonymous function whose body is a single call passes its caller's nargout on: two targets ask max for both of its outputs
f = @(v) max(v); [m, i] = f([1 5 2]); disp(i)
