% covers: 12 - (Scope, QA D13) num2str of a matrix with a precision and with a format is still one char row per matrix row
% Only the rows are asserted, not the column widths inside them: str2num
% reads the second row back as the matrix's second row.
p = num2str([1 2; 3 4], 3);
disp(size(p, 1))
disp(str2num(p(2, :)))
f = num2str([1 2; 3 4], '%d ');
disp(size(f, 1))
disp(str2num(f(2, :)))
