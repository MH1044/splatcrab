% covers: 13 - eig of a matrix holding NaN ends with a clean exit, never a panic (101), an abort (134) or a hang
% The spec allows a clean error or a NaN result, with SplatCrab's own text.
% This case holds the clean error, which the Error: prefix places on line 5,
% counting this covers line as line 1.
eig([NaN 1; 1 1])
