% covers: 8 - permute with a dimension order that skips a dimension, holding 4 but not 3, is refused; exit 1
A = reshape(1:24, 2, 3, 4);
permute(A, [1 2 4])
