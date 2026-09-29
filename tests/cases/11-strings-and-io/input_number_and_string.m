% covers: 7 - input evaluates the typed text as an expression, input(prompt, 's') returns it as text, and each prompt is written with no newline after it
% The two lines typed are the .stdin file's, which the harness pipes in.
x = input('n: ');
s = input('name: ', 's');
fprintf('%d %s\n', x * 2, s)
