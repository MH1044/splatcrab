% covers: rethrow lasterr warning assert isequal - lasterr is empty before any error and is the last message raised after one, caught or not
x = lasterr;
disp(isempty(x))
try, error('MyPkg:id', 'boom %d', 1), catch, end
disp(lasterr)
