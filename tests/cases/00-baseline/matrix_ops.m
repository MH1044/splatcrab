% covers: matrix literal, *, transpose, .*, backslash solve, inv, det
% NOTE: MATLAB prints det(A) as -2.0000 (roundoff); SplatCrab prints -2,
% NOTE: because its det lands exactly on -2. Cycle 08 rewrites det (verify first).
A = [1 2; 3 4]
B = A * A
A'
A .* A
x = A \ [5; 6]
inv(A)
det(A)
