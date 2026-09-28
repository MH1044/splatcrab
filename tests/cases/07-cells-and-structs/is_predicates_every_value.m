% covers: 15 - (Scope, is*) the class and shape predicates answer for a handle, an MException, a cell and a struct instead of refusing them
% A handle and an MException are 1x1 and never empty (the Scope bullet), so
% isscalar is true; none of the four values is char, numeric or logical.
% Each result is a logical, which disp prints four wide.
f = @sin; disp(isscalar(f)); disp(ischar(f)); disp(isnumeric(f))
try, error('a:b', 'm'), catch e, end; disp(isscalar(e)); disp(isempty(e)); disp(isnumeric(e))
c = {1, 2}; disp(isvector(c)); disp(isempty(c)); disp(isnumeric(c))
s.a = 1; disp(isscalar(s)); disp(ischar(s)); disp(islogical(s))
