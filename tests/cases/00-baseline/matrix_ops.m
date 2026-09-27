% covers: matrix literal, *, transpose, .*, backslash solve, inv, det
% NOTE: MATLAB prints det(A) as -2.0000 (roundoff); SplatCrab prints -2.
% NOTE: MATLAB pads integer columns differently for values >= 1000.
A = [1 2; 3 4]
B = A * A
A'
A .* A
x = A \ [5; 6]
inv(A)
det(A)
