% covers: 16 - bare toc works after a bare tic, and toc(t) works with or without one
% NOTE: A comparison is a logical in MATLAB, and disp of a logical is four
% NOTE: characters wide ("   1"), as it has been here since cycle 02. Adding 0
% NOTE: makes the value a double, so the expected "     1" below is the double
% NOTE: width, in MATLAB and here alike.
% NOTE: toc is never a bare statement here: MATLAB then prints the elapsed
% NOTE: time, which varies from run to run.
t0 = tic;
disp(0 + (toc(t0) >= 0))
tic;
x = toc;
disp(0 + (x >= 0))
t = tic;
disp(0 + (toc(t) >= 0))
y = toc;
disp(0 + (y >= 0))
