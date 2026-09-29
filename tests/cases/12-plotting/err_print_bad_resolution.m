% covers: 10 - (Design notes, error texts) print with a resolution that is not a number is refused before any file is written, exit 1
% The options are read before the figure is drawn, so no file named after
% the case is left behind.
plot(1:3);
print('-dpng', '-rabc', 'err_print_bad_resolution.png');
