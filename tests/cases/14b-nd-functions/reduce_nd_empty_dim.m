% covers: 1 and 2 - a reduction along a dimension of size 0 reduces nothing into each result element, that dimension becoming 1 and every other size kept: sum 0, prod 1, mean NaN, any false and all true, while max and min give an empty result along it and cumsum keeps every size; the default dimension of a 2x0x3 is the first
E = zeros(2, 0, 3);
S = sum(E, 2);
disp(size(S))
disp(S(:)')
P = prod(E, 2);
disp(size(P))
disp(P(:)')
M = mean(E, 2);
disp(size(M))
disp(M(:)')
Y = any(E, 2);
disp(size(Y))
disp(class(Y))
disp(Y(:)')
W = all(E, 2);
disp(size(W))
disp(W(:)')
disp(size(max(E, [], 2)))
disp(size(min(E, [], 2)))
disp(size(cumsum(E, 2)))
disp(size(sum(E)))
