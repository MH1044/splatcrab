% covers: 6 - bar of three values saves an SVG holding three bar rectangles
% Item 6 as the spec writes it, the file named after the case. The SVG is
% read back and deleted before anything is asserted about it.
bar([1 2 3]);
saveas(gcf, 'bar_rects.svg');
s = fileread('bar_rects.svg');
delete('bar_rects.svg');
disp(numel(strfind(s, '<rect class="bar"')))
