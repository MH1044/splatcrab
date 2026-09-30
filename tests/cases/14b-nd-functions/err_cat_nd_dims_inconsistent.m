% covers: 10 - cat along dimension 3 of arrays that differ in dimension 4, a dimension past the first array's ndims being 1, is refused with the inconsistent-dimensions message; exit 1
cat(3, zeros(2, 2, 2), zeros(2, 2, 1, 2))
