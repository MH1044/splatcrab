% covers: 10 - (regression) a nonsingular square solve is unchanged by the least-squares rewrite
% 4.5 is not an integer, so the column prints with four decimals and -4 as
% -4.0000 whatever its last bits, as 00-baseline/matrix_ops has it.
disp([1 2; 3 4] \ [5; 6])
