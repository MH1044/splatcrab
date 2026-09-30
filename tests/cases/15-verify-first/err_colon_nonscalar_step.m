% covers: 10 - a colon whose step is not a scalar, 1:[1 2]:5, is a clean error naming the step; exit 1
% The text is the spec's. disp runs first, so the error comes at run time;
% the Error: prefix counts this covers line as line 1.
disp(numel([1 2]))
x = 1:[1 2]:5
