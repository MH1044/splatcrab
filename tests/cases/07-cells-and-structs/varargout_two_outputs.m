% covers: 9 - a function whose output is varargout returns the cells it assigns as separate outputs
[a, b] = mv(); disp(b)
function varargout = mv()
varargout{1} = 1; varargout{2} = 2;
end
