% covers: 4 - arrayfun over two arrays of different sizes is a clean error, exit 1
% The text is SplatCrab's own, pinned here: MATLAB's first sentence as
% recalled, not MATLAB-sourced. The Error: prefix names line 5, counting
% this covers line as line 1; a panic would exit 101.
disp(arrayfun(@(a, b) a * b, [1 2], [3 4 5]))
