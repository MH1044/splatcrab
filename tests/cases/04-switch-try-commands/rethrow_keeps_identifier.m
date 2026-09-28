% covers: 6 - rethrow raises the error unchanged: the outer catch sees its identifier and its message
try, try, error('MyPkg:myid', 'Value %d bad', 7), catch e, rethrow(e), end, catch e2, disp(e2.identifier), disp(e2.message), end
