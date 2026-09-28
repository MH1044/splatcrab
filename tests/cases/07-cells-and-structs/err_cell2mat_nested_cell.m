% covers: 10 - (Scope, cell2mat) cell2mat of a cell holding a cell is a clean error, exit 1
% SplatCrab's own text (the spec's Messages), pinned here.
% Line 5 prints first, so the error comes at run time; the Error: prefix
% counts this covers line as line 1.
disp(1)
x = cell2mat({{1}})
