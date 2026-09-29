% covers: 11 - close all as a command closes every figure, so the next figure is number 1 again
% Item 11's second half as the spec writes it, with two figures open first
% so that close all has something to close. No file is written.
figure;
figure;
close all;
figure;
disp(gcf)
