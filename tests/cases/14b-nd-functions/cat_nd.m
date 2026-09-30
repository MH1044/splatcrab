% covers: 9 - cat joins its arrays along any dimension, one past every argument's ndims making pages, every other dimension agreeing; an empty argument beside a nonempty one is omitted, and when every argument is empty the result is the empty their sizes give; the class follows the bracket rule and the result is complex if any argument is; cat(dim) with no arrays is [] and with one array is that array
C = cat(3, [1 2; 3 4], [5 6; 7 8])
A = reshape(1:24, 2, 3, 4);
B = cat(1, A, A);
disp(size(B))
disp(isequal(B(1:2, :, :), A))
disp(isequal(B(3:4, :, :), A))
G = cat(3, A, [1 2 3; 4 5 6]);
disp(size(G))
disp(G(:, :, 5))
disp(isequal(G(:, :, 1:4), A))
disp(size(cat(2, ones(2, 2), ones(2, 3))))
disp(size(cat(3, ones(2, 2), [])))
disp(size(cat(1, zeros(0, 5), ones(2, 3))))
disp(size(cat(3, zeros(2, 0), zeros(2, 0))))
F = cat(4, 1, 2);
disp(size(F))
disp(F(:)')
disp(size(cat(3, A, A, A)))
d = cat(3, 'ab', 'cd')
disp(class(d))
disp(class(cat(3, true, false)))
disp(class(cat(3, true, 2)))
disp(class(cat(3, 'a', 66)))
disp(isreal(cat(3, 1, 2i)))
x = cat(3)
disp(isequal(cat(2, A), A))
