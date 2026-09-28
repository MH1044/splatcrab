% covers: 4 - arrayfun whose first argument is not a function handle is a clean error, exit 1
% The text is SplatCrab's own, not MATLAB-sourced, in the form of the
% existing Argument N to 'f' messages. Line 4 counts this covers line as 1.
disp(arrayfun(1, [1 2]))
