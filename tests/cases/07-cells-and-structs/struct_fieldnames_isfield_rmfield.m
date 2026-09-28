% covers: 4 - field assignment builds a struct; fieldnames lists its fields in the order they were made, isfield tests one, and rmfield returns the struct without it
% isfield returns a logical, which disp prints four wide.
s.a = 1; s.b = 'hi'; disp(s.a + 1); f = fieldnames(s); disp(f{2}); disp(isfield(s, 'a')); s = rmfield(s, 'a'); disp(isfield(s, 'a'))
