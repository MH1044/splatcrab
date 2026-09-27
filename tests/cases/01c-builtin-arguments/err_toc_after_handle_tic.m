% covers: 16 - t = tic does not count as a bare tic: toc(t) works, a bare toc still errors
t = tic;
disp(0 + (toc(t) >= 0))
x = toc
