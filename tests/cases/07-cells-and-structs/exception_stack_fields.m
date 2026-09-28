% covers: 7 - (Scope, e.stack) a caught error's stack is a struct array with fields file, name and line, its first element the innermost frame
% The stack holds cycle 05's error trace, innermost first, and the trace
% names a function alone (Known deviations), so element 1 is g's frame at
% the line of its error call: line 11, counting this covers line as line 1.
% The script's own frame is not an element, and file is '' for a function
% local to the script (the spec's Design notes), so there is one element.
try, g(), catch e, end
disp(class(e.stack)); disp(isfield(e.stack, 'file')); disp(isfield(e.stack, 'name')); disp(isfield(e.stack, 'line'))
disp(e.stack(1).name); disp(e.stack(1).line); disp(numel(e.stack)); disp(isempty(e.stack(1).file))
function g()
error('a:b', 'm')
end
