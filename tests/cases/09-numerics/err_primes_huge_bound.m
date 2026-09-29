% covers: 10 - (Scope, invariant 6) primes(1e12) holds far more primes than any array may, and ends in a clean error, exit 1, never an allocator abort (134) or a hang
% There are about 3.8e10 primes below 1e12, past the 2^28-element cap, so
% no answer can be returned. The sieve is judged by args::check_shape
% before it is allocated, so the text is that size error's.
p = primes(1e12);
