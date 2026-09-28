% covers: 14 - (Scope, addpath/rmpath) addpath of a folder that does not exist and rmpath of one not on the path are warnings, and the script runs on
% SplatCrab's own texts, as recalled from MATLAB and not confirmed; the
% folder is named as it was given. Exit 0: a warning is not an error.
addpath('no_such_folder');
rmpath('no_such_folder');
disp(1)
