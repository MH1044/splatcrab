% covers: 14 - (invariant 6) an anonymous function handed itself recurses with no user function in between, and still ends in a clean error, exit 1, never a stack overflow (134)
% f can never name itself, since capture happens at creation, but it can be
% passed itself. Every anonymous call counts against the recursion limit
% (Design notes, Calls), one per level, so the limit of 500 is met long
% before the nesting limit of 10,000. Line 7 counts this covers line as
% line 1.
f = @(g, n) g(g, n + 1); f(f, 1)
