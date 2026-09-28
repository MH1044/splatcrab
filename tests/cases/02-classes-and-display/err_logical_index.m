% covers: 16 - a logical index is a clean error until cycle 03 (QA D6)
% Today x > 0 is read as the positions 1 1 1, so this prints 5 5 5 and exits 0.
% Cycle 03 replaces the error with logical indexing and this case with its own.
x = [5 6 7];
x(x > 0)
