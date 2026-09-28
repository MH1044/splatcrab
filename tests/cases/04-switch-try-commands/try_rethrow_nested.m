% covers: 6 - a catch with no name swallows a builtin's error, and rethrow raises a caught error again to the outer try
try, x = [1 2] * [3 4]; catch, disp('caught'), end; try, try, error('in'), catch e, rethrow(e), end, catch e2, disp(['outer: ' e2.message]), end
