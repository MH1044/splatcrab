% covers: 10 - (Scope, gcd) gcd of 2.5 is refused with a clean error, exit 1, never a loop in Euclid's algorithm
% The text is SplatCrab's own, recorded in the spec's Design notes.
% NaN and Inf are refused the same way, so no input can make it spin.
g = gcd(2.5, 5);
