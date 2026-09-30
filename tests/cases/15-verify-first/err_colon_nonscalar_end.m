% covers: 10 - a colon whose end is not a scalar, 1:size(ones(3, 4)), is a clean error naming the end; exit 1
% The text is the spec's. disp runs first, so the error comes at run time;
% the Error: prefix counts this covers line as line 1.
disp(size(ones(3, 4)))
x = 1:size(ones(3, 4))
