% covers: 15 - regexp of (a*)*b over 100,000 a's finds no match and returns at once: the engine runs in linear time and never backtracks (invariant 6)
% A backtracking matcher takes exponential time on this pattern. The harness
% kills a case at ten seconds; toc < 5 asserts the bound inside that.
tic;
r = regexp(repmat('a', 1, 100000), '(a*)*b', 'match');
disp(isempty(r))
disp(toc < 5)
