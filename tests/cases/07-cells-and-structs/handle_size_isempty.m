% covers: 15 - size and isempty answer for a function handle, which is 1x1 and never empty
% isempty returns a logical, which disp prints four wide.
f = @sin; disp(size(f)); disp(isempty(f))
