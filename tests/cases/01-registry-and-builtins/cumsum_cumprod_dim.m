% covers: 17 - cumsum and cumprod honour the dimension argument
A = [1 2; 3 4];
disp(cumsum(A))
disp(cumsum(A, 1))
disp(cumsum(A, 2))
disp(cumprod(A))
disp(cumprod(A, 2))
disp(cumsum([1 2 3], 1))
disp(cumsum([1 2 3], 2))
disp(cumprod([1 2 3], 1))
