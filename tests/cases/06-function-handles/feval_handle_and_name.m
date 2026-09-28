% covers: 5 - feval calls a handle, and still calls a function named by a char
disp(feval(@(x) x + 1, 1)); disp(feval('sin', 0))
