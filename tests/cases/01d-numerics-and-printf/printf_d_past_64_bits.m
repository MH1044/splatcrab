% covers: 15 - %d prints an integral value past 2^63 in full, not the i64 clamp
% The old code cast to i64, so anything at or above 2^63 printed
% 9223372036854775807. The expected text below is the exact decimal value of
% the double 1e30, which is what "print the value" means: the nearest double
% to 1e30 is 1000000000000000019884624838656, not a round power of ten.
fprintf('%d\n', 1e30);
