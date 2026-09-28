% covers: 5 - (Scope, struct) struct with a field name and no value is a clean error, exit 1
% SplatCrab's own text (the spec's Messages), pinned here.
% Line 5 prints first, so the error comes at run time; the Error: prefix
% counts this covers line as line 1.
disp(1)
s = struct('a')
