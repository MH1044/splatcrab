% covers: 13 - chol of a matrix holding NaN ends with a clean exit, never a panic (101), an abort (134) or a hang
% The spec allows a clean error or a NaN result, with SplatCrab's own text.
% This case holds the clean error, which the Error: prefix places on line 5,
% counting this covers line as line 1.
chol([NaN 0; 0 1])
