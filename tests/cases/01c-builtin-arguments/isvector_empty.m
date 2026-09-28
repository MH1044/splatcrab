% covers: 15 - a 1x0 or 0x1 array is a vector (QA D18); a 0x0 is not
% NOTE: isvector returns a logical in MATLAB, and disp of a logical is four
% NOTE: characters wide ("   1"), as it has been here since cycle 02. Adding 0
% NOTE: makes the value a double, so the expected "     1" below is the double
% NOTE: width, in MATLAB and here alike.
disp(0 + [isvector(zeros(1, 0)) isvector(zeros(0, 1)) isvector([]) isvector(5) isvector(zeros(2, 0)) isvector([1 2 3])])
disp(0 + [isvector([1; 2]) isvector(ones(2)) isvector(zeros(0, 3))])
