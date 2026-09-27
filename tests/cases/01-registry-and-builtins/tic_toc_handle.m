% covers: 5 - toc(t) with a handle from tic gives a non-negative elapsed time
t = tic;
x = toc(t);
disp(x >= 0)
