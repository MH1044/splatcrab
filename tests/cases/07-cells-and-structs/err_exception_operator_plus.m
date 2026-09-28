% covers: 15 - (Scope, operators) a binary operator on an MException is a clean error, exit 1, in the same sentence with the class MException
% The Scope bullet puts this sentence in place of cycle 04's generic text
% for binary operators. disp(class(e)) runs first; the Error: prefix names
% line 6, counting this covers line as line 1.
try, error('a:b', 'm'), catch e, end; disp(class(e))
e + 1
