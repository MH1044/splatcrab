% covers: 5 - the element-wise math of an N-D array keeps its shape, each result checked by its size and its (:)' row: abs, sqrt (self-checked), floor, mod, rem, round, isnan (a logical) and sign, then atan2 of two 1x1x2 arrays (self-checked) and hypot broadcasting a 1x1x2 against a 2x1
A = reshape(1:24, 2, 3, 4);
B = abs(-A);
disp(size(B))
disp(B(:)')
B = sqrt(A .^ 2);
disp(size(B))
disp(isequal(B, A))
B = floor(A / 5);
disp(size(B))
disp(B(:)')
B = mod(A, 5);
disp(size(B))
disp(B(:)')
B = rem(-A, 5);
disp(size(B))
disp(B(:)')
B = round(A / 7);
disp(size(B))
disp(B(:)')
B = isnan(A ./ 0 - Inf);
disp(size(B))
disp(class(B))
disp(B(:)')
B = sign(A - 12);
disp(size(B))
disp(B(:)')
T = atan2(ones(1, 1, 2), ones(1, 1, 2));
disp(size(T))
disp(all(abs(T(:) - pi / 4) < 1e-12))
H = hypot(reshape([4 0], 1, 1, 2), [0; 3]);
disp(size(H))
disp(H(:)')
