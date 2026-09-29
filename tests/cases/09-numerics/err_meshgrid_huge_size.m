% covers: 11 - (Scope, check_shape) meshgrid of two 1e5-element vectors asks for 1e10 elements: the clean size error, exit 1, never an allocator abort (134)
% Each operand is small enough to build; the grid is not. The text is
% args::check_shape's; the Error: prefix names line 5, counting this covers
% line as line 1.
[X, Y] = meshgrid(1:1e5, 1:1e5);
