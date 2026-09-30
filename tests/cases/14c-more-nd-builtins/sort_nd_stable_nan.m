% covers: 1 - sort of an N-D array is stable, equal elements keeping their order along dimension 3 ascending and descending; NaN sorts last ascending and first descending along dimension 3, its index following it; and along a dimension past ndims the array is returned with indices of 1
[~, I] = sort(ones(1, 1, 3));
disp(I(:)')
[~, I] = sort(ones(1, 1, 3), 'descend');
disp(I(:)')
[~, I] = sort(cat(3, 2, 1, 2, 1), 3);
disp(I(:)')
[s, I] = sort(cat(3, NaN, 1, 2));
disp(s(:)')
disp(I(:)')
[s, I] = sort(cat(3, NaN, 1, 2), 'descend');
disp(s(:)')
disp(I(:)')
A = reshape(1:24, 2, 3, 4);
[B, I] = sort(A, 5);
disp(isequal(B, A))
disp(all(I(:) == 1))
disp(size(I))
