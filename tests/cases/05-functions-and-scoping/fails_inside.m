function y = fails_inside(x)
% Helper for err_eval_error_line_outermost: it fails on its own line 4, so
% the protocol's line, 2 in the submitted code, cannot be this file's.
y = x + no_such_name_zz;
end
