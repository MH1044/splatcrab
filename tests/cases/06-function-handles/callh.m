function [a, b] = callh(h, v)
% Helper for handle_local_from_path_file: a function file on the path that
% calls the handle it is given, then calls twist by name, which inside this
% file is its own subfunction. It has no covers line and no .out, so the
% harness never runs it as a case.
a = h(v);
b = twist(v);
end

function r = twist(x)
r = -1;
end
