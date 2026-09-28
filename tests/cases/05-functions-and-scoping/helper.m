function y = helper(x)
% Helper for err_subfunction_private and subfunction_before_script_function:
% a function file whose subfunction twice is visible only inside this file.
y = twice(x);
end

function y = twice(x)
y = 2 * x;
end
