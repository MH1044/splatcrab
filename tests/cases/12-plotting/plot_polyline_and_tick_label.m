% covers: 1 - plot(x, y) saves an SVG holding one polyline, and the value 9 appears as a text of its own
% Item 1 as the spec writes it, the file named after the case. The SVG is
% read back and deleted before anything is asserted about it, so no failure
% below can leave it behind. The second line is a logical, four wide since
% cycle 02.
plot(1:3, [1 4 9]);
saveas(gcf, 'plot_polyline_and_tick_label.svg');
s = fileread('plot_polyline_and_tick_label.svg');
delete('plot_polyline_and_tick_label.svg');
disp(numel(strfind(s, '<polyline')))
disp(~isempty(strfind(s, '>9<')))
