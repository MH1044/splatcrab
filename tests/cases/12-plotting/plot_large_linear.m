% covers: 12 - plot of 100,000 points saves an SVG holding one polyline, well inside the harness's timeout: the work is linear in the points
% Item 12 as the spec writes it, the file named after the case. The bound
% asserted is the harness's own ten-second timeout, in the debug build. The
% SVG is read back and deleted before anything is asserted about it.
plot(1:1e5);
saveas(gcf, 'plot_large_linear.svg');
s = fileread('plot_large_linear.svg');
delete('plot_large_linear.svg');
disp(numel(strfind(s, '<polyline')))
