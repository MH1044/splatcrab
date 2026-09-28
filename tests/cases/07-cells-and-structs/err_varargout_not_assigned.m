% covers: 9 - (Scope, varargout) asking for more outputs than varargout holds is a clean error at the call, exit 1
% Cycle 05's output-not-assigned sentence, naming the missing varargout{3}; the
% check is made at the call, so the prefix names the script's line.
% Line 6 prints first, so the error comes at run time; the Error: prefix
% counts this covers line as line 1.
disp(1)
[a, b, c] = mv()
function varargout = mv()
varargout{1} = 1; varargout{2} = 2;
end
