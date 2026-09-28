% covers: 6 - an infinite range step gives one element, not an empty
% MATLAB's documented count for j:i:k is fix((k-j)/i), which is fix(4/Inf) = 0,
% and a count of 0 is one element: the start. `range` returns early on a
% non-finite step today and gives a 1x0. Cycle 01d refused the infinite end
% point and deliberately left the step alone; this is that remainder.
disp(size(1:Inf:5))
disp(1:Inf:5)
