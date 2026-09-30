% covers: 8 - (Scope, text functions and char arrays of several rows) the places item 8 does not reach: strrep's third argument, strfind's second argument and its cell, regexp's expression and its cell, regexprep's text and replacement, strjoin's delimiter as a cell, strsplit's delimiter as a char and as a cell, and a cell element of three dimensions to lower and strrep; each gives the message that place gives a value that is not a char; an N-D char argument keeps the N-D message
% The texts follow the spec's two forms, Argument N for an argument and
% Every element for an element of a cell, N being the argument's place;
% each is caught and printed on one line.
try, x = strrep('abc', 'a', ['x'; 'y']); catch e, disp(e.message); end
try, x = strfind('abc', ['a'; 'b']); catch e, disp(e.message); end
try, x = strfind({['ab'; 'cd']}, 'a'); catch e, disp(e.message); end
try, x = regexp('abc', ['a'; 'b'], 'match'); catch e, disp(e.message); end
try, x = regexp({['ab'; 'cd']}, 'a', 'match'); catch e, disp(e.message); end
try, x = regexprep(['ab'; 'cd'], 'a', 'z'); catch e, disp(e.message); end
try, x = regexprep('abc', 'a', ['x'; 'y']); catch e, disp(e.message); end
try, x = strjoin({'a', 'b'}, {['-'; '+']}); catch e, disp(e.message); end
try, x = strsplit('a b', ['-'; '+']); catch e, disp(e.message); end
try, x = strsplit('a b', {['-'; '+']}); catch e, disp(e.message); end
try, x = lower({cat(3, 'a', 'b')}); catch e, disp(e.message); end
try, x = strrep({cat(3, 'a', 'b')}, 'a', 'z'); catch e, disp(e.message); end
try, x = upper(cat(3, 'a', 'b')); catch e, disp(e.message); end
try, x = strrep(cat(3, 'a', 'b'), 'a', 'z'); catch e, disp(e.message); end
