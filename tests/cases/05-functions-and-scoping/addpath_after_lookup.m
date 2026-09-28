% covers: 14 - a lookup made before a path change does not outlive it: the builtin max, then shadow/max.m, then the builtin again
disp(max([1 5 2])); addpath('shadow'); disp(max([1 5 2])); rmpath('shadow'); disp(max([1 5 2]))
