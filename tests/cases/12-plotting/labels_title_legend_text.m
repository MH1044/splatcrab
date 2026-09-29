% covers: 4 - xlabel, ylabel, title and legend each put their text in the saved SVG as a text of its own
% Item 4 as the spec writes it, the file named after the case. The SVG is
% read back and deleted before anything is asserted about it.
plot(1:2);
xlabel('t');
ylabel('y');
title('T');
legend('a');
saveas(gcf, 'labels_title_legend_text.svg');
s = fileread('labels_title_legend_text.svg');
delete('labels_title_legend_text.svg');
fprintf('%d %d %d %d\n', ~isempty(strfind(s, '>t<')), ~isempty(strfind(s, '>y<')), ~isempty(strfind(s, '>T<')), ~isempty(strfind(s, '>a<')))
