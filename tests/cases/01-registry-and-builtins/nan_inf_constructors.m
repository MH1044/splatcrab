% covers: 25 - NaN(2) and Inf(2, 3) fill a matrix instead of discarding the arguments
disp(size(NaN(2)))
disp(all(all(isnan(NaN(2)))))
disp(NaN(2))
disp(size(Inf(2, 3)))
disp(all(all(isinf(Inf(2, 3)))))
disp(size(NaN(2, 3)))
disp(size(NaN))
disp(size(Inf))
