% covers: 7 - a second argument to input other than 's' is a clean error, exit 1, before the prompt is written or a line is read
% The .stdin holds a line, so the error is the argument's and not the end of input.
% The text is SplatCrab's own.
x = input('n: ', 'x');
