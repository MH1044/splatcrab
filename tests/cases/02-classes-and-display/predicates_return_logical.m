% covers: 17 - the predicate builtins return logical
% Cycle 03's x(isnan(x)) needs these to be masks, not doubles of ones and zeros.
disp(class(any(1)))
disp(class(all(1)))
disp(class(isnan(1)))
disp(class(isinf(1)))
disp(class(isfinite(1)))
disp(class(isempty(1)))
disp(class(isscalar(1)))
disp(class(isvector(1)))
disp(class(ischar('a')))
