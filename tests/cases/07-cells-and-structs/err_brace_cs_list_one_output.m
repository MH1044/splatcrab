% covers: 12 - assigning a brace cs-list of two elements to one name is a clean error, exit 1
% The spec's recorded text. disp(numel(c)) runs first; the Error: prefix
% names line 5, counting this covers line as line 1.
c = {1, 2}; disp(numel(c))
y = c{:}
