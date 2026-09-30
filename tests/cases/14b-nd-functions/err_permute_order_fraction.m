% covers: 8 - permute with a dimension order holding a number that is not an integer is refused; exit 1
A = reshape(1:24, 2, 3, 4);
permute(A, [1 2 3.5])
