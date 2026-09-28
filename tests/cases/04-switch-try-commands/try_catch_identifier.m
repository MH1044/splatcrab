% covers: 5 - error('id', fmt, ...) sets e.identifier and formats e.message
try, error('MyPkg:myid', 'Value %d bad', 7), catch e, disp(e.identifier), disp(e.message), end
