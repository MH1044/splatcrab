% covers: 12 - arrayfun over N-D arrays of one size gives a result of that size, one element for each position, the function called in column-major order; a logical result keeps its class, and an empty N-D array gives the empty of its size
A = reshape(1:24, 2, 3, 4);
B = arrayfun(@(x) x * 2, A);
disp(size(B))
disp(B(2, 3, 4))
disp(isequal(B, 2 * A))
disp(isequal(arrayfun(@(x, y) x + y, A, A), 2 * A))
Q = arrayfun(@(x) x ^ 2, reshape(1:3, 1, 1, 3));
disp(size(Q))
disp(Q(:)')
L = arrayfun(@(x) x > 12, A);
disp(class(L))
disp(isequal(L, A > 12))
disp(size(arrayfun(@(x) x, zeros(2, 0, 3))))
R = arrayfun(@arrayfun_order_helper, reshape([1 2 3 4 5 6 7 8], 2, 2, 2));
fprintf('\n');
disp(size(R))
