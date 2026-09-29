% covers: 11 - (Scope, check_shape) logspace with a huge point count is the clean size error linspace gives, exit 1
% The text is args::check_shape's, as cycle 01's err_huge_size_linspace pins
% it; the Error: prefix names line 4, counting this covers line as line 1.
x = logspace(0, 1, 1e10);
