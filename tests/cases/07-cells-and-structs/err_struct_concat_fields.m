% covers: 13 - (Scope, concatenation) brackets joining structs with different field names are a clean error, exit 1
% SplatCrab's own text (the spec's Messages), pinned here.
% Line 5 prints first, so the error comes at run time; the Error: prefix
% counts this covers line as line 1.
a.x = 1; b.y = 2; disp(a.x)
c = [a, b]
