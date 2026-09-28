% covers: 8 - (Scope, cellfun) cellfun given a double where a cell belongs is a clean error, exit 1
% SplatCrab's own text (the spec's Messages), pinned here.
% Line 5 prints first, so the error comes at run time; the Error: prefix
% counts this covers line as line 1.
disp(1)
r = cellfun(@numel, 5)
