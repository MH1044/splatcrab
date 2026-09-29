% covers: 2 - with hold on, a second plot adds a line to the axes: the saved SVG holds two polylines
% Item 2 as the spec writes it, the file named after the case. The SVG is
% read back and deleted before anything is asserted about it.
hold on;
plot(1:3, 1:3);
plot(1:3, 2:4);
hold off;
saveas(gcf, 'hold_on_two_lines.svg');
s = fileread('hold_on_two_lines.svg');
delete('hold_on_two_lines.svg');
disp(numel(strfind(s, '<polyline')))
