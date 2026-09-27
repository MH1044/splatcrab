% covers: 1 - eps(x) is the spacing at abs(x), element-wise over an array, and eps('double') is eps
% NOTE: A comparison is a logical in MATLAB, and disp of a logical is four
% NOTE: characters wide ("   1"). SplatCrab has no logical class until cycle 02
% NOTE: and prints six wide. Adding 0 makes the value a double in both, so the
% NOTE: expected "     1" below is MATLAB's own output, and stays so after 02.
fprintf('%g %g %g %g\n', eps(1), eps(2), eps(-2), eps(1e10));
fprintf('%g %g %g %g\n', eps(0), eps(1e308), eps(Inf), eps(NaN));
disp(eps([1 2; 4 8]) / eps)
disp(0 + (eps('double') == eps))
fprintf('%g %g %g\n', eps(0.5), eps(1e-310), eps(-Inf));
disp(size(eps(ones(2, 3))))
disp(0 + (eps(1e308) == 2^971))
