% covers: 5 - scatter of three points saves an SVG holding three circles
% Item 5 as the spec writes it, the file named after the case. The SVG is
% read back and deleted before anything is asserted about it.
scatter([1 2 3], [3 1 2]);
saveas(gcf, 'scatter_circles.svg');
s = fileread('scatter_circles.svg');
delete('scatter_circles.svg');
disp(numel(strfind(s, '<circle')))
