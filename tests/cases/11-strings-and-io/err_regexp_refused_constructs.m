% covers: 15 - every construct the linear-time engine refuses is a clean error with SplatCrab's text: lookahead and lookbehind, possessive quantifiers, atomic groups, conditionals, inline flags and named backreferences
% NOTE: MATLAB matches all of these; SplatCrab refuses them, the deviation
% NOTE: the spec records. The texts are SplatCrab's own.
pats = {'(?=a)', '(?<!a)b', 'a*+', '(?>a)', '(?(1)a|b)', '(?i)a', '(?<x>a)\k<x>'};
for k = 1:numel(pats)
  try
    regexp('aab', pats{k});
    disp('matched');
  catch e
    disp(e.message);
  end
end
regexprep('ab', '(?<=a)b', 'x')
