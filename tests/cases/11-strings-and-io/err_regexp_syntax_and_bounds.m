% covers: 15 - a pattern that does not parse, or is past the engine's bounds on nesting, repetition counts and size, is a clean error naming the fault, exit 1
% The texts are SplatCrab's own. The last pattern nests 300 groups, past the
% bound of 250 that keeps the parser's recursion finite.
pats = {'*a', '(a', 'a)', '[a', 'a\', 'a{3,2}', '(?<1a>x)', '(?<ab', '\x{zz}', '\x{110000}', '[z-a]', 'a{1001}', [repmat('(', 1, 300) 'a' repmat(')', 1, 300)]};
for k = 1:numel(pats)
  try
    regexp('aab', pats{k});
    disp('matched');
  catch e
    disp(e.message);
  end
end
regexprep('ab', 'a{2,1}', 'x')
