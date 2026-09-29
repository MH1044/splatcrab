% covers: 6 - (Scope, QA D16 f and cycle 01d's bound) a * width taken from an argument is bounded like a written one: an absurd one is a clean error, exit 1, never a two-gigabyte pad
% The bound is judged before a single space is written, so this returns at
% once. The width reaches the formatter from the argument list, not from the
% format text, which is the path the 01d bound did not have to cover.
% NOTE: the .err substring is 01d's, deliberately short, for the reason given
% in tests/cases/01d-numerics-and-printf/err_printf_precision_f.m.
s = sprintf('%*d', 2147483647, 1);
