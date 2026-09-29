% covers: 5 - str2num reads its constants from the builtins and never calls a file: with a pi.m returning 42 on the path pi is 42, and str2num('pi') is still 3.1416
% The helper is str2num_shadow/pi.m, in a folder of its own: beside the other
% cases of this module it would shadow pi for every one of them.
addpath('str2num_shadow');
disp(pi)
fprintf('%.4f\n', str2num('pi'));
fprintf('%.4f\n', str2num('[pi; 2*pi]'));
rmpath('str2num_shadow');
fprintf('%.4f\n', pi);
