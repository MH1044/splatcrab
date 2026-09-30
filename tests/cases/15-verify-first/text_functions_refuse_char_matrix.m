% covers: 8 - where a text function takes a character vector or a cell of character vectors, a char array of several rows, or of three dimensions, is refused with the message that place gives a value that is not a char; strtrim refuses a cell element of three dimensions with the N-D message
% The texts are the spec's (S8, S9 and Scope, text functions and char
% arrays of several rows); each is caught and printed on one line.
try, x = upper({['ab'; 'cd']}); catch e, disp(e.message); end
try, x = lower({'ab'.'}); catch e, disp(e.message); end
try, x = strrep(['ab'; 'cd'], 'a', 'z'); catch e, disp(e.message); end
try, x = strrep('abc', ['a'; 'b'], 'z'); catch e, disp(e.message); end
try, x = strrep({['ab'; 'cd']}, 'a', 'z'); catch e, disp(e.message); end
try, x = strfind(['ab'; 'cd'], 'a'); catch e, disp(e.message); end
try, x = regexp(['ab'; 'cd'], 'a', 'match'); catch e, disp(e.message); end
try, x = regexprep('abc', ['a'; 'b'], 'z'); catch e, disp(e.message); end
try, x = regexprep({['ab'; 'cd']}, 'a', 'z'); catch e, disp(e.message); end
try, x = strjoin({'a', ['b'; 'c']}); catch e, disp(e.message); end
try, x = strjoin({'a', 'b'}, ['-'; '+']); catch e, disp(e.message); end
try, x = strcat({['ab'; 'cd']}, 'z'); catch e, disp(e.message); end
try, x = strsplit(['a b'; 'c d']); catch e, disp(e.message); end
try, x = strtok(['a b'; 'c d']); catch e, disp(e.message); end
try, x = upper({cat(3, 'a', 'b')}); catch e, disp(e.message); end
try, x = strtrim({cat(3, 'a', 'b')}); catch e, disp(e.message); end
