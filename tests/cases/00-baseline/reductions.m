% covers: sum, mean, prod, max, min, any, all, cumsum with and without a dimension
A = [1 2; 3 4];
sum(A(:))
sum(A)
sum(A, 2)
mean([1 2 3 4])
prod([1 2 3 4])
max([3 9 2])
min([3 9 2])
max(A)
any([0 0 1])
all([1 1 0])
cumsum([1 2 3])
cumprod([1 2 3])
