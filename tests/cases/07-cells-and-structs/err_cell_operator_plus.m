% covers: 12 - a binary operator on a cell is a clean error, exit 1, in MATLAB's R2020a sentence
% The spec's recorded text, which replaced the older Undefined function
% 'plus' wording. disp(c{1}) runs first; the Error: prefix names line 6,
% counting this covers line as line 1.
c = {1}; disp(c{1})
c + 1
