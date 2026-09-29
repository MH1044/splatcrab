% covers: 14 - (Scope, command syntax) grid on and grid off work as commands: the saved SVG has grid lines after grid on and none after grid off, and keeps its line
% The spec's SVG vocabulary draws the grid as <line class="grid"> at the
% ticks when it is on. Each file is named after the case, read back and
% deleted before anything is asserted.
plot(1:2);
grid on
saveas(gcf, 'grid_on_as_command.svg');
s = fileread('grid_on_as_command.svg');
delete('grid_on_as_command.svg');
grid off
saveas(gcf, 'grid_on_as_command.svg');
t = fileread('grid_on_as_command.svg');
delete('grid_on_as_command.svg');
fprintf('%d %d %d\n', ~isempty(strfind(s, 'class="grid"')), isempty(strfind(t, 'class="grid"')), ~isempty(strfind(t, '<polyline')))
