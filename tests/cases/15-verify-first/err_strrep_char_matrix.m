% covers: 8 - strrep of a char array of two rows is refused, its first argument being no character vector; exit 1
% The text is the spec's. disp runs first, so the error comes at run time;
% the Error: prefix counts this covers line as line 1.
m = ['ab'; 'cd']; disp(size(m))
x = strrep(m, 'a', 'z')
