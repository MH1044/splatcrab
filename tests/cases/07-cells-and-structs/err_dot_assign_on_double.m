% covers: 12 - a field assignment into a variable holding a double is a clean error, exit 1
% The text is the spec's recorded one. disp(x) runs first, which proves the
% error comes at run time, not from the parser; the Error: prefix names
% line 6, counting this covers line as line 1.
x = 1; disp(x)
x.a = 2
