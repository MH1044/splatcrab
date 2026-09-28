% covers: 13 - exist is 1 for a variable, 5 for a builtin and 0 for a name that is none of them
% max is a SplatCrab builtin, so 5; this directory holds no max.m, and
% shadow/ is not on the path here. nosuch is no variable, file or builtin.
x = 1; disp(exist('x')); disp(exist('max')); disp(exist('nosuch'))
