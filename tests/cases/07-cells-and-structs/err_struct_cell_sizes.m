% covers: 5 - (Scope, struct) struct given cell values of two different sizes, neither 1x1, is a clean error, exit 1
% SplatCrab's own text (the spec's Messages), pinned here.
% Line 5 prints first, so the error comes at run time; the Error: prefix
% counts this covers line as line 1.
disp(1)
s = struct('a', {1, 2}, 'b', {1, 2, 3})
