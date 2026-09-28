% covers: 4 - arrayfun whose function returns a non-scalar is a clean error, exit 1: without 'UniformOutput', false (cycle 07) every result must be a scalar
% The text is SplatCrab's own, pinned here: MATLAB's as recalled, on one
% line, not MATLAB-sourced. The Error: prefix names line 5, counting this
% covers line as line 1; a panic would exit 101.
disp(arrayfun(@(x) [x x], [1 2]))
