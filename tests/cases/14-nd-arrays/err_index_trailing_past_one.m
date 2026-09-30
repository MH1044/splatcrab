% covers: 5 - a subscript past an N-D array's dimensions must select position 1; exit 1
A = reshape(1:24, 2, 3, 4);
A(1, 1, 1, 2)
