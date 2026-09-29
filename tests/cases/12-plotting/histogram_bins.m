% covers: 7 - histogram with three bins saves an SVG holding three bar rectangles
% Item 7 as the spec writes it, the file named after the case. The SVG is
% read back and deleted before anything is asserted about it.
histogram([1 1 2 3 3 3], 3);
saveas(gcf, 'histogram_bins.svg');
s = fileread('histogram_bins.svg');
delete('histogram_bins.svg');
disp(numel(strfind(s, '<rect class="bar"')))
