% covers: 10 - a colon whose start is not a scalar, S1's own example [1 2 3]:2:10, is a clean error naming the start; exit 1
% The text is the spec's. disp runs first, so the error comes at run time;
% the Error: prefix counts this covers line as line 1.
disp(numel([1 2 3]))
x = [1 2 3]:2:10
