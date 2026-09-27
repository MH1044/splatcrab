% covers: 15 - a 1x0 or 0x1 array is a vector (QA D18); a 0x0 is not
% NOTE: isvector returns a logical in MATLAB, and disp of a logical is four
% NOTE: characters wide ("   1"). SplatCrab has no logical class until cycle 02
% NOTE: and prints six wide. Adding 0 makes the value a double in both, so the
% NOTE: expected "     1" below is MATLAB's own output, and stays so after 02.
disp(0 + [isvector(zeros(1, 0)) isvector(zeros(0, 1)) isvector([]) isvector(5) isvector(zeros(2, 0)) isvector([1 2 3])])
disp(0 + [isvector([1; 2]) isvector(ones(2)) isvector(zeros(0, 3))])
