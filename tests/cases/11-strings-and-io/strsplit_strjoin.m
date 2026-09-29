% covers: 2 - strsplit splits at a delimiter into a cell of char pieces, and strjoin joins a cell with one
c = strsplit('a,b,c', ',');
disp(numel(c))
disp(c{3})
disp(strjoin({'a', 'b'}, '-'))
