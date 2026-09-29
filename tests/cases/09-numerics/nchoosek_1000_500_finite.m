% covers: 10 - (Scope, nchoosek) nchoosek(1000, 500), near 2.7e299, is computed by the multiplicative formula without overflowing to Inf, exit 0
% A self-check: the value is roundoff, so only its size is asserted.
c = nchoosek(1000, 500); disp(c > 2.7e299 && c < 2.71e299)
