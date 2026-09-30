% covers: 5 - fliplr and flipud of an N-D array flip each page on its own, fliplr along dimension 2 and flipud along dimension 1, keeping the size and the class of a char or logical array, an empty array keeping its size
A = reshape(1:24, 2, 3, 4);
F = fliplr(A);
disp(size(F))
disp(F(:, :, 2))
disp(F(:, :, 4))
U = flipud(A);
disp(size(U))
disp(U(:, :, 4))
disp(U(:, :, 1))
c = cat(3, 'ab', 'cd');
x = fliplr(c);
disp(x(:, :, 2))
disp(class(x))
disp(size(x))
disp(isequal(flipud(c), c))
L = flipud(A > 12);
disp(class(L))
disp(isequal(L, flipud(A) > 12))
disp(size(fliplr(zeros(2, 0, 3))))
disp(size(flipud(zeros(0, 2, 3))))
