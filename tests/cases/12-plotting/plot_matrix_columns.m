% covers: 9 - plot of a 3x2 matrix draws one line per column: the saved SVG holds two polylines
% Item 9 as the spec writes it, the file named after the case. The SVG is
% read back and deleted before anything is asserted about it.
plot([1 2; 3 4; 5 6]);
saveas(gcf, 'plot_matrix_columns.svg');
s = fileread('plot_matrix_columns.svg');
delete('plot_matrix_columns.svg');
disp(numel(strfind(s, '<polyline')))
