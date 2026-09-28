function h = makeh()
% Helper for handle_to_subfunction_returned: a function file on the path
% that returns a handle to its own subfunction. It has no covers line and no
% .out, so the harness never runs it as a case.
h = @hidden;
end

function r = hidden(x)
r = 3 * x;
end
