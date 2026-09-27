% covers: 14 - any ignores NaN (QA D11), as the MATLAB any page says ("any ignores elements of A that are NaN")
% NOTE: the spec says Octave gives any(NaN) = 1, but the installed Octave 8.4
% NOTE: gives 0, and agrees with every value below.
% NOTE: any and all return a logical in MATLAB, and disp of a logical is four
% NOTE: characters wide ("   1"). SplatCrab has no logical class until cycle 02
% NOTE: and prints six wide. Adding 0 makes the value a double in both, so the
% NOTE: expected "     1" below is MATLAB's own output, and stays so after 02.
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
