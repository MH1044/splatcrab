% covers: 2 - the string functions refuse a cell holding something other than text, a strjoin delimiter cell of the wrong length, cells of different sizes, char arrays of different row counts, a bad precision or count and a mat2str of a cell, each a clean error, exit 1
% The texts are SplatCrab's own, but for strcmp's, which follows MATLAB's.
try, strjoin({1, 'a'}); catch e, disp(e.message); end
try, strjoin({'a', 'b', 'c'}, {'-'}); catch e, disp(e.message); end
try, strcmp({'a', 'b'}, {'a', 'b', 'c'}); catch e, disp(e.message); end
try, strcat(['a'; 'b'], ['c'; 'd'; 'e']); catch e, disp(e.message); end
try, strcat({'a', 'b'}, {'a', 'b', 'c'}); catch e, disp(e.message); end
try, mat2str(1, 0); catch e, disp(e.message); end
try, mat2str({1}); catch e, disp(e.message); end
strncmp('a', 'b', -1)
