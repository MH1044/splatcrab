% covers: 1 - (Design notes, error texts) saveas to an extension that is neither svg nor png is refused before any file is written, exit 1
% The format is judged before anything is drawn, so no file named after
% the case is left behind.
plot(1:3);
saveas(gcf, 'err_saveas_unsupported_format.jpg');
