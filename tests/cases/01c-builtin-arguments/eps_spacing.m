% covers: 1 - eps(x) is the spacing at abs(x), element-wise over an array, and eps('double') is eps
% NOTE: A comparison is a logical in MATLAB, and disp of a logical is four
% NOTE: characters wide ("   1"), as it has been here since cycle 02. Adding 0
% NOTE: makes the value a double, so the expected "     1" below is the double
% NOTE: width, in MATLAB and here alike.
fprintf('%g %g %g %g\n', eps(1), eps(2), eps(-2), eps(1e10));
fprintf('%g %g %g %g\n', eps(0), eps(1e308), eps(Inf), eps(NaN));
disp(eps([1 2; 4 8]) / eps)
disp(0 + (eps('double') == eps))
fprintf('%g %g %g\n', eps(0.5), eps(1e-310), eps(-Inf));
disp(size(eps(ones(2, 3))))
disp(0 + (eps(1e308) == 2^971))
