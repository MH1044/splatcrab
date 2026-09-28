% covers: 6 - bare toc returns a value, so it is usable in an expression
% NOTE: this asks for a value rather than the spec bullet's class() form, which
% NOTE: did not exist when the case was written. Asking for a value is enough:
% NOTE: a builtin that returned nothing would raise "Too many output
% NOTE: arguments." on either line below.
% NOTE: toc is never run as a bare statement here, because MATLAB prints
% NOTE: "Elapsed time is ... seconds." for that form and the number varies.
tic;
x = toc;
disp(x >= 0)
disp(isscalar(toc))
