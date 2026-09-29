% covers: 12 - (Scope, bounded work) a constant at the largest double, and a manual limit farther past the data than a double can resolve, still give increasing limits and an SVG whose every coordinate is a number
% Both used to write NaN coordinates: widening the constant overflowed to
% Inf, and xlim([1e300 Inf]) put the automatic end at 1e300 + 2, which is
% 1e300. Each file is named after the case, read back and deleted before
% anything is asserted.
plot([1.7976931348623157e308 1.7976931348623157e308]);
v = ylim;
saveas(gcf, 'extreme_limits_stay_numbers.svg');
s = fileread('extreme_limits_stay_numbers.svg');
delete('extreme_limits_stay_numbers.svg');
plot(1:3);
xlim([1e300 Inf]);
w = xlim;
saveas(gcf, 'extreme_limits_stay_numbers.svg');
t = fileread('extreme_limits_stay_numbers.svg');
delete('extreme_limits_stay_numbers.svg');
fprintf('%d %d %d %d\n', v(1) < v(2), w(1) < w(2), isempty(strfind(s, 'NaN')), isempty(strfind(t, 'NaN')))
