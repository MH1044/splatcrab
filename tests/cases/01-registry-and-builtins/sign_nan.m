% covers: 23 - sign(NaN) is NaN
disp(sign(NaN))
disp(sign([-3 0 5]))
fprintf('%g %g %g\n', sign([-Inf 0 Inf]));
disp(isnan(sign(NaN)))
