% covers: 15 - a character class is built once into sorted ranges and bisected, so a class of ten thousand b's that a counted repetition copies 19,000 times, and a class of ten thousand ranges over 100,000 units, each return at once; a class of more ranges than the program may hold is "The regular expression is too large.", exit 1
% Each range of a class counts as one instruction against regex::MAX_PROGRAM,
% 20,000; the last class holds 20,001 code units, no two adjacent.
tic
r = regexp(repmat('a', 1, 1e5), ['(?:[' repmat('b', 1, 10000) ']{1000}){19}'], 'match');
disp(isempty(r))
cls = char(19968 + 2 * (0:9999));
m = regexp(cls(mod(0:99999, 10000) + 1), ['[' cls ']+'], 'match');
disp(numel(m{1}))
disp(toc < 5)
regexp('a', ['[' char(256 + 2 * (0:20000)) ']'])
