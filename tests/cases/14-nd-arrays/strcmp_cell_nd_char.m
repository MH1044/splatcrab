% covers: 12 - strcmp and strcmpi compare an N-D char held in a cell by every dimension, so the same characters in two shapes are not the same text
a = {reshape('abcd', 1, 1, 4)};
b = {reshape('abcd', 1, 1, 2, 2)};
disp(strcmp(a, b))
disp(strcmp(a, a))
disp(strcmpi(a, {reshape('ABCD', 1, 1, 2, 2)}))
disp(strcmpi(a, {reshape('ABCD', 1, 1, 4)}))
