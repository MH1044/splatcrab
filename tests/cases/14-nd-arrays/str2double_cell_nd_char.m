% covers: 12 - a builtin reading a cell judges an N-D element itself: str2double reads a cell's N-D char as no text, NaN as for a char matrix, however few its rows, and a character vector beside it still as its number
v = str2double({reshape('1234', 1, 2, 2), '1234'});
disp(size(v))
disp(isnan(v(1)))
disp(v(2) == 1234)
