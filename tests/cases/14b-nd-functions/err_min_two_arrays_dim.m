% covers: 3 - min of two N-D arrays with a dimension is refused rather than dropping the second array
A = reshape(1:24, 2, 3, 4);
min(A, A, 3)
