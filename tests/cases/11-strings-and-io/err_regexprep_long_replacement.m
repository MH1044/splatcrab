% covers: 11 - regexprep reads a replacement of 50,000 '$<' in linear time, builds 100,000 replacements of 20,000 mostly empty tokens from the parts that write, and refuses 20,000 '$0' per match over 100,000 matches without walking every part, naming the size asked for, exit 1
% '$<' with no name after it is text. The subject of the second is 99,990
% x's and 10 y's: the 20,000 uses of $1 are empty at every x.
tic
r = regexprep('abc', 'b', repmat('$<', 1, 5e4));
disp(numel(r))
disp(r(1:7))
r = regexprep([repmat('x', 1, 99990) repmat('y', 1, 10)], '(y)|(x)', ['<' repmat('$1', 1, 2e4) '$2>']);
disp(numel(r))
disp(r(1:6))
try, regexprep(repmat('a', 1, 1e5), 'a', repmat('$0', 1, 2e4)); catch e, disp(toc < 5); rethrow(e); end
