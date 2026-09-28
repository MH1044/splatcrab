% covers: 14 - any ignores NaN (QA D11), as the MATLAB any page says ("any ignores elements of A that are NaN")
% NOTE: the spec says Octave gives any(NaN) = 1, but the installed Octave 8.4
% NOTE: gives 0, and agrees with every value below.
% NOTE: any and all return a logical in MATLAB, and disp of a logical is four
% NOTE: characters wide ("   1"), as it has been here since cycle 02. Adding 0
% NOTE: makes the value a double, so the expected "     1" below is the double
% NOTE: width, in MATLAB and here alike.
disp(0 + any(NaN))
disp(0 + any([NaN 0]))
disp(0 + any([NaN 1]))
disp(0 + any([NaN; 0], 1))
disp(0 + any([NaN 0; 0 2], 2)')
disp(0 + all(NaN))
disp(0 + any([0 NaN; NaN 0]))
disp(0 + any([NaN NaN 3]))
disp(0 + any([NaN; NaN; 0]))
disp(0 + all([NaN 1]))
