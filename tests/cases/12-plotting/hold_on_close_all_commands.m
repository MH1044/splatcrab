% covers: 14 - hold on and close all work as command syntax: two plots under hold on save two polylines, and close all ends the script cleanly
% Item 14 as the spec writes it, the file named after the case. The SVG is
% read back and deleted before anything is asserted about it.
hold on;
plot(1:2);
plot(2:3);
saveas(gcf, 'hold_on_close_all_commands.svg');
s = fileread('hold_on_close_all_commands.svg');
delete('hold_on_close_all_commands.svg');
disp(numel(strfind(s, '<polyline')))
close all
