% covers: 8 - ipermute with a dimension order shorter than ndims of the array is refused with the order message; exit 1
A = reshape(1:24, 2, 3, 4);
ipermute(A, [1 2])
