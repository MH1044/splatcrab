% covers: 8 - the line spec 'r--' draws the line red and dashed: the SVG holds stroke="#ff0000" and a stroke-dasharray
% Item 8 as the spec writes it, the file named after the case. The SVG is
% read back and deleted before anything is asserted about it.
plot(1:3, 1:3, 'r--');
saveas(gcf, 'plot_line_spec_red_dashed.svg');
s = fileread('plot_line_spec_red_dashed.svg');
delete('plot_line_spec_red_dashed.svg');
fprintf('%d %d\n', ~isempty(strfind(s, 'stroke="#ff0000"')), ~isempty(strfind(s, 'stroke-dasharray')))
