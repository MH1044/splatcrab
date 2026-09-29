% covers: 14 - (Scope, ismember of cells) [tf, loc] = ismember of a cell of char in a cell of char is a mask and each match's position, 0 where none
[tf, loc] = ismember({'b', 'z'}, {'a', 'b'}); disp(tf); disp(loc)
