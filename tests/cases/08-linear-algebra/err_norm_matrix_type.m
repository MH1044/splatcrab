% covers: 8 - (Scope) a matrix norm of an order other than 1, 2, Inf or 'fro' is a clean error with SplatCrab's own text, exit 1
% The first line shows the 'inf' spelling is accepted; the 3-norm of a
% matrix is not.
disp(norm([1 2; 3 4], 'inf'))
norm([1 2; 3 4], 3)
