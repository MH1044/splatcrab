% covers: 6 - (Scope, QA D16 f and cycle 01d's bound) a * precision taken from an argument is bounded like a written one: 65536 is a clean error, exit 1, never the formatter's panic
% Rust holds a formatter's precision in a u16, so 65536 reaching it unbounded
% panics with exit 101, as %.65536f did before cycle 01d.
% NOTE: the .err substring is 01d's, deliberately short, for the reason given
% in tests/cases/01d-numerics-and-printf/err_printf_precision_f.m.
s = sprintf('%.*f', 65536, 1);
