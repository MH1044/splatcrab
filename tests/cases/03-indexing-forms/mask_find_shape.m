% covers: 19 - a mask indexes as find(mask) would: a row mask on a matrix gives a row, a matrix mask a column
A = [1 2; 3 4];
disp(A(logical([1 0 0 1])))
disp(A(logical([1 0; 0 1])))
