% covers: 3 - subplot(2, 1, k) makes two axes in one figure: the saved SVG holds two class="axes" groups
% Item 3 as the spec writes it, the file named after the case. The SVG is
% read back and deleted before anything is asserted about it.
subplot(2, 1, 1);
plot(1:2);
subplot(2, 1, 2);
plot(1:3);
saveas(gcf, 'subplot_two_axes.svg');
s = fileread('subplot_two_axes.svg');
delete('subplot_two_axes.svg');
disp(numel(strfind(s, 'class="axes"')))
