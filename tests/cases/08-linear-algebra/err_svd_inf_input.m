% covers: 13 - svd of a matrix holding Inf ends with a clean exit, never a panic (101), an abort (134) or a hang
% The spec allows a clean error or a NaN result, with SplatCrab's own text.
% This case holds the clean error, which the Error: prefix places on line 5,
% counting this covers line as line 1.
svd([Inf 0; 0 1])
