% covers: 14 - addpath('shadow') puts shadow/max.m ahead of the builtin max, and rmpath('shadow') takes it away again
% shadow is resolved against the working directory, this case's directory.
addpath('shadow'); disp(max([1 5 2])); rmpath('shadow'); disp(max([1 5 2]))
