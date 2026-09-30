% covers: 8 - permute's dimension order is a row, so a column holding each of 1 to n once is refused; exit 1
A = reshape(1:24, 2, 3, 4);
permute(A, [3; 1; 2])
