% covers: 17 - indexed growth names the size asked for, 1x1e+300, not the usize clamp; exit 1, not 101
% The spec routes growth through args::check_shape, so the full sentence is the
% one 01c-builtin-arguments/err_size_overflow_named records for zeros(1e300).
x = [];
x(1e300) = 1
