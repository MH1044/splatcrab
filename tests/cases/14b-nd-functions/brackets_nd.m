% covers: 11 - brackets join N-D arrays by cat's rule, a comma along dimension 2 and a semicolon along dimension 1, every other dimension agreeing and an empty beside them omitted, the class by the bracket rule and complex if any element is
A = reshape(1:24, 2, 3, 4);
disp(size([A, A]))
disp(size([A; A]))
B = [A, A];
disp(B(1, 4, 1))
disp(isequal(B(:, 1:3, :), A))
disp(isequal(B(:, 4:6, :), A))
V = [A; A];
disp(isequal(V(3:4, :, :), A))
disp(size([A, zeros(2, 1, 4)]))
disp(isequal([A, []], A))
disp(isequal([[]; A], A))
D = [reshape(1:4, 1, 2, 2), reshape(5:8, 1, 2, 2)];
disp(size(D))
disp(D(:)')
c = 'ab';
c(:, :, 2) = 'cd';
e = [c, c];
disp(class(e))
disp(size(e))
disp(e(:)')
L = [true(1, 1, 2); false(1, 1, 2)];
disp(class(L))
disp(size(L))
disp(L(:)')
Z = [A, A * 1i];
disp(isreal(Z))
disp(size(Z))
