% covers: 8 - broadcasting across every dimension, two dimensions agreeing when equal or one is 1 and a dimension past an operand's ndims being 1: a column against a 2x2x2, a 1x1x4 against a 2x3x4, a 1x1x3 against a row
A = reshape(1:8, 2, 2, 2);
B = A - [10; 20];
disp(size(B))
disp(B(:)')
C = zeros(2, 3, 4) + ones(1, 1, 4);
disp(size(C))
disp(isequal(C, ones(2, 3, 4)))
D = reshape(1:3, 1, 1, 3) .* [1 2];
disp(size(D))
disp(D(:)')
