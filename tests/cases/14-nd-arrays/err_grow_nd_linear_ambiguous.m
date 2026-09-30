% covers: 6 - a linear subscript past the end of an N-D array is the ambiguous-growth error; exit 1
A = zeros(2, 2, 2);
A(9) = 1;
