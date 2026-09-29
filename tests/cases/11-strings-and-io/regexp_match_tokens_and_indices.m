% covers: 11 - (Scope, a hand-rolled regexp) regexp's one-based start indices, its 'match' and 'tokens' outputs, and regexprep of an escaped metacharacter
% '12' starts at 3 and '345' at 7. The second match of the tokens pattern is
% y=22, whose two tokens are y and 22.
disp(regexp('ab12cd345', '\d+'))
m = regexp('ab12cd345', '\d+', 'match');
disp(numel(m))
disp(m{2})
t = regexp('x=1, y=22', '([a-z])=(\d+)', 'tokens');
disp(numel(t))
u = t{2};
disp(u{1})
disp(u{2})
disp(regexprep('a.b.c', '\.', '-'))
