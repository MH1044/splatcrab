% covers: 8 - the element-wise operators, unary minus and plus, and *, / and \ with a scalar reach every element of an N-D array and keep its shape
A = reshape(1:8, 2, 2, 2);
B = A + 1;
disp(size(B))
disp(B(:)')
B = A .* A;
disp(size(B))
disp(B(:)')
B = -A;
disp(size(B))
disp(B(:)')
B = 2 * A;
disp(size(B))
disp(B(:)')
B = A / 2;
disp(size(B))
disp(B(:)')
disp(isequal(A * 2, 2 * A))
disp(isequal(2 \ A, A / 2))
disp(isequal(A ./ 2, A / 2))
disp(isequal(2 .\ A, A / 2))
disp(isequal(A .^ 2, A .* A))
disp(isequal(+A, A))
disp(isequal(A - A, zeros(2, 2, 2)))
