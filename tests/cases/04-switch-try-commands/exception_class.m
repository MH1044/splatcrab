% covers: 16 - a caught error is an MException
try, error('a:b', 'msg'), catch e, disp(class(e)), end
