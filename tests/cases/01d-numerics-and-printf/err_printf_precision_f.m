% covers: 5 - printf bounds its precision, so %.65536f is a clean error
% Rust holds a formatter's precision in a u16, so 65536 used to panic with
% "Formatting argument out of range", exit 101.
% NOTE: the .err substring is deliberately short. The spec leaves the exact
% bound and its wording to the implementation; what this case pins is exit 1
% rather than 101, and a message that names the width or the precision.
fprintf('%.65536f', 1);
