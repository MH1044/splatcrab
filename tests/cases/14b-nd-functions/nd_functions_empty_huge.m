% covers: 1 to 7, 9, 11 and 12 - the functions taught N-D take an empty array whose other dimensions are huge, reducing, scanning, rearranging, tiling and joining it by its sizes at once, with no step for each position along a huge dimension
n = 2^40;
E = zeros(0, n, n);
disp(isequal(size(sum(E, 2)), [0 1 n]))
disp(isequal(size(prod(E, 3)), [0 n]))
disp(isequal(size(mean(E, 2)), [0 1 n]))
Y = any(E, 2);
disp(isequal(size(Y), [0 1 n]))
disp(class(Y))
disp(isequal(size(all(E, 3)), [0 n]))
disp(isequal(size(max(E, [], 3)), [0 n]))
[m, i] = min(E, [], 2);
disp(isequal(size(m), [0 1 n]))
disp(isequal(size(i), [0 1 n]))
disp(isequal(size(max(E, [], 1)), [0 n n]))
disp(isequal(size(cumsum(E, 3)), [0 n n]))
disp(isequal(size(cumprod(E, 2)), [0 n n]))
disp(isequal(size(abs(E)), [0 n n]))
disp(isequal(size(mod(E, 3)), [0 n n]))
disp(isequal(size(permute(E, [3 1 2])), [n 0 n]))
disp(isequal(size(squeeze(zeros(1, 0, 1, n))), [0 n]))
disp(isequal(size(cat(3, E, E)), [0 n 2 * n]))
disp(isequal(size([E, E]), [0 2 * n n]))
disp(isequal(size([E; E]), [0 n n]))
disp(isequal(size(repmat(E, 1, 1, 2)), [0 n 2 * n]))
disp(isequal(size(repmat(zeros(0, 1), 1, n, n)), [0 n n]))
disp(numel(repmat(E, 1, 1, 2)))
