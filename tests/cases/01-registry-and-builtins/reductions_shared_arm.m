% covers: 8 - sum, prod, mean, any and all with and without a dimension argument
A = [1 2; 3 4];
disp(sum(A))
disp(sum(A, 1))
disp(sum(A, 2))
disp(prod(A))
disp(prod(A, 2))
disp(mean(A))
disp(mean(A, 2))
B = [1 0; 0 0];
disp(any(B))
disp(any(B, 2))
disp(all(B))
disp(all(B, 2))
