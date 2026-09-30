% covers: 6 - an assignment past the end grows a matrix into new pages and a fourth dimension, zeros filling what is new and the old pages kept
B = zeros(2, 2);
B(:, :, 2) = [1 2; 3 4];
disp(size(B))
disp(B(:, :, 2))
B(1, 1, 3) = 9;
disp(size(B))
disp(B(2, 2, 3))
disp(B(1, 1, 3))
B(1, 1, 1, 2) = 5;
disp(size(B))
disp(B(1, 1, 1, 2))
disp(B(:, :, 2, 1))
disp(B(:, :, 3, 2))
