% covers: 15 - sort puts NaN last and leaves the finite values correctly ordered
disp(sort([5 4 NaN 2 1]))
fprintf('%g %g %g\n', sort([NaN 1 NaN]));
fprintf('%g %g %g\n', sort([NaN -Inf Inf]));
disp(sort([5 4 2 1]))
