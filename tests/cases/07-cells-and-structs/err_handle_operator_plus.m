% covers: 15 - a binary operator on a function handle is a clean error, exit 1, in MATLAB's R2020a sentence with the class function_handle
% The spec's recorded text. disp(class(f)) runs first; the Error: prefix
% names line 5, counting this covers line as line 1.
f = @sin; disp(class(f))
f + 1
