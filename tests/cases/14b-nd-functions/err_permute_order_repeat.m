% covers: 8 - permute with a dimension order that repeats a dimension is refused; exit 1
A = reshape(1:24, 2, 3, 4);
permute(A, [1 1 2])
