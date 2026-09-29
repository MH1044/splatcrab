% covers: 10 - (Scope, gcd, invariant 6) gcd of NaN is refused with a clean error, exit 1, never a hang in Euclid's algorithm
% The text is SplatCrab's own, recorded in the spec's Design notes.
g = gcd(NaN, 3);
