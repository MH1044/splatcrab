% covers: 9 - (Scope, varargout) a function that sets varargout to a double is a clean error at the call, exit 1
% SplatCrab's own text (the spec's Messages), pinned here; the check is made at the call, so the prefix names the script's line.
% Line 5 prints first, so the error comes at run time; the Error: prefix
% counts this covers line as line 1.
disp(1)
x = vo()
function varargout = vo()
varargout = 1;
end
