% covers: 12 - arrayfun of an N-D array beside a matrix of a different size is refused with the same-size message; exit 1
A = reshape(1:24, 2, 3, 4);
arrayfun(@(x, y) x + y, A, ones(2, 3))
