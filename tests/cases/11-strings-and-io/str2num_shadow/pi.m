function y = pi
% Helper for str2num_constants_ignore_the_path. It shadows the builtin pi
% only once addpath('str2num_shadow') has put this folder on the path.
y = 42;
end
