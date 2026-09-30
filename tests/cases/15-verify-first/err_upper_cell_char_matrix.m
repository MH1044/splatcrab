% covers: 8 - upper of a cell holding a char array of two rows is refused, the element being no character vector; exit 1
% The text is the spec's. disp runs first, so the error comes at run time;
% the Error: prefix counts this covers line as line 1.
c = {['ab'; 'cd']}; disp(size(c{1}))
x = upper(c)
