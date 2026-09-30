% covers: 1, 3, 7, 9 and 12 - a dimension far past ndims costs nothing extra: sum along dimension 1e300 returns the array, max along dimension 2^53 gives it with indices of 1, permute by an order of a million dimensions returns the array, and cat along dimension 100000 and repmat by 100001 counts make arrays of that many dimensions, which squeeze folds back to a column
A = reshape(1:24, 2, 3, 4);
disp(isequal(sum(A, 1e300), A))
[m, i] = max(A, [], 2^53);
disp(isequal(m, A))
disp(isequal(i, ones(2, 3, 4)))
disp(isequal(permute(A, 1:1e6), A))
C = cat(1e5, 1, 2);
disp(ndims(C))
disp(size(C, 1e5))
disp(C(:)')
R = repmat(7, [ones(1, 1e5) 2]);
disp(ndims(R))
disp(R(:)')
disp(size(squeeze(R)))
