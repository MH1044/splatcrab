% covers: 7 - deletion from an N-D array with two subscripts that are not colons is the null-assignment error; exit 1
A = reshape(1:24, 2, 3, 4);
A(1, 2, :) = [];
