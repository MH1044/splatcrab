% covers: ranges with default, fractional, and negative steps; empty ranges; linspace
% NOTE: MATLAB prints an exact zero in a fixed-point row as 0, not 0.0000.
% NOTE: MATLAB prints an empty range as "1x0 empty double row vector"; cycle 02 fixes both.
a = 0:0.25:1
b = 5:-1:1
c = 3:1
d = linspace(0, 1, 5)
e = 1:5
