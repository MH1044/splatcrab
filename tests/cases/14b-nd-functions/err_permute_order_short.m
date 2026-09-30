% covers: 8 - permute with a dimension order shorter than ndims of the array is refused; exit 1
A = reshape(1:24, 2, 3, 4);
permute(A, [1 2])
