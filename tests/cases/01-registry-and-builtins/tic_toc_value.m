% covers: 6 - bare toc returns a value, so it is usable in an expression
% NOTE: the class() form in the spec bullet waits for cycle 13. Asking for a
% NOTE: value is enough: a builtin that returned nothing would raise
% NOTE: "Too many output arguments." on either line below.
% NOTE: toc is never run as a bare statement here, because MATLAB prints
% NOTE: "Elapsed time is ... seconds." for that form and the number varies.
tic;
x = toc;
disp(x >= 0)
disp(isscalar(toc))
