% covers: 1 - (Scope, the builtins) xlim, ylim and axis set the limits they are given and return them, and clf removes every axes, exit 0
% xlim, ylim and axis with no argument return the limits the axes shows,
% as MATLAB's do, so each setting is read straight back. After clf the
% figure has no axes, so its SVG holds no class="axes" group. The file is
% named after the case, read back and deleted before anything is asserted.
plot(1:3);
xlim([0 4]);
disp(xlim)
ylim([0 10]);
disp(ylim)
axis([1 2 3 4]);
disp(axis)
clf;
saveas(gcf, 'limits_axis_clf_run.svg');
s = fileread('limits_axis_clf_run.svg');
delete('limits_axis_clf_run.svg');
disp(numel(strfind(s, 'class="axes"')))
