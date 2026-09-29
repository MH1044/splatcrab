% covers: 12 - num2str of a matrix is one char row per matrix row with MATLAB's column widths, a column right-aligned (QA D13)
s = num2str([1 2; 3 4]);
disp(size(s))
disp(s)
disp(num2str([1; 22]))
