% covers: 6 - the bound covers %e too, one value below the %f limit
% 65535 fits the u16 that makes %.65536f panic, so the old note "65535 is fine"
% held for %f alone: %e was recorded reaching its own assertion, ndigits > 0.
% Whether it still panics or merely builds a 65541-character string, a
% precision this large is absurd and must be refused, and the bound is one
% bound for every conversion.
% NOTE: the .err substring is deliberately short, for the reason given in
% err_printf_precision_f.m. Exit 1, never 101, is the assertion.
fprintf('%.65535e\n', 1);
