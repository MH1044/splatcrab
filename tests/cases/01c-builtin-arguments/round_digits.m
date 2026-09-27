% covers: 9 - round(x, n) to a multiple of 10^-n for any integer n, ties away from zero; 'decimals' and 'significant'
% NOTE: A comparison is a logical in MATLAB, and disp of a logical is four
% NOTE: characters wide ("   1"). SplatCrab has no logical class until cycle 02
% NOTE: and prints six wide. Adding 0 makes the value a double in both, so the
% NOTE: expected "     1" below is MATLAB's own output, and stays so after 02.
% NOTE: The last three value lines hold the guards of "How to build it" to the
% NOTE: Scope's rule, the nearest multiple of 10^-n. That rule leaves NaN and
% NOTE: Inf unchanged for a negative n too, and makes round(1e300, -2) about
% NOTE: 1e300, not 0: only a 10^|n| that overflows (round(5, -400)) gives 0.
fprintf('%.4f %.4f %.4f %.4f\n', round(pi, 2), round(pi, 3), round(-pi, 1), round(pi, 2, 'decimals'));
fprintf('%d %d\n', round(863178137, -2), round(1234, -1));
fprintf('%g %g %g\n', round([1253 1.345 120.44], 2, 'significant'));
disp(0 + (round(pi, 20) == pi))
disp(0 + (round(1e307, 2) == 1e307))
disp(round(5, -400))
disp(round(2.5, 0))
fprintf('%d %d\n', round(-2.5, 0), round(-1253, 2, 'significant'));
fprintf('%.4f %.4f\n', round([1.234 5.678], 1));
fprintf('%.4f %.4f\n', round([1 2] / 3, 2));
fprintf('%d %d\n', round(1234.5678, -2), round(987654, 2, 'significant'));
fprintf('%g\n', round(-0.0045678, 3, 'significant'));
fprintf('%g %g %g %g\n', round(0, 3, 'significant'), round(Inf, 2), round(NaN, 2), round(-Inf, 2, 'significant'));
fprintf('%g %g\n', round(NaN, -1), round(-Inf, -2));
disp(0 + (abs(round(1e300, -2) / 1e300 - 1) < 1e-12))
disp(size(round(ones(2, 3) / 3, 2)))
