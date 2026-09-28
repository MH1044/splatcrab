% covers: 15 - isa and numel answer for a caught MException: isa(e, 'MException') is true and numel is 1
% isa returns a logical, which disp prints four wide.
try, error('a:b', 'm'), catch e, end; disp(isa(e, 'MException')); disp(numel(e))
