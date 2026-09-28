% covers: 11 - a NaN or an Inf does not stop the integer column format
% Today one non-finite element forces the whole row to four decimals:
% `    1.0000    2.0000       NaN`. MATLAB keeps the integer columns, six
% characters wide, and right-aligns NaN, Inf and -Inf in them.
%
% The last line is the other half, and guards against over-correcting: a row
% that is genuinely fixed-point still prints its NaN in the ten-character
% column, not the six-character one.
disp([1 2 NaN])
disp([1 Inf])
disp([NaN Inf -Inf 1])
disp([1.5 NaN])
