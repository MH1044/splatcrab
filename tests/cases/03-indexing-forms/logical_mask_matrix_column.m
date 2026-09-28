% covers: 3 - a matrix mask on a matrix selects in column-major order and gives a column
% A(:) is 1 4 7 2 5 8 3 6 9, so the elements above 5 come out as 7 8 6 9; the
% transpose turns that column into a row for disp.
A = [1 2 3; 4 5 6; 7 8 9];
disp(A(A > 5)')
