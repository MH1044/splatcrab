% covers: 8 - permute's dimension order is a row of positive integers, so a char row is refused whatever its codes, even codes that would be a valid order; exit 1
A = reshape(1:24, 2, 3, 4);
permute(A, char([2 1 3]))
