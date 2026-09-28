function m = max(v)
% Helper for addpath_shadow and addpath_after_lookup. It shadows the builtin
% max only once addpath('shadow') has put this directory on the path.
m = 42;
end
