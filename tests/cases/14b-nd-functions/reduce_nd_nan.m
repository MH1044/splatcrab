% covers: 1, 2, 3 and 4 - the reductions keep today's NaN handling on an N-D array: sum, prod and mean along a dimension holding a NaN give NaN there, cumsum carries it on, any ignores NaN and all counts it as nonzero, and max and min ignore it, their index naming the element chosen, unless every element is NaN
N = reshape([1 2 NaN 4 5 6], 1, 2, 3);
disp(sum(N, 3))
disp(prod(N, 3))
disp(mean(N, 3))
C = cumsum(N, 3);
disp(size(C))
disp(C(:)')
disp(any(NaN(1, 1, 2)))
disp(all(NaN(1, 1, 2)))
[m, i] = max(N, [], 3);
disp(m)
disp(i)
[m, i] = min(N, [], 3);
disp(m)
disp(i)
disp(max(NaN(1, 1, 2)))
