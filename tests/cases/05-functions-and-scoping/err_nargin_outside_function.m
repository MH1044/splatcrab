% covers: 3 - (Scope, nargin/nargout) outside every function nargin and nargout are an error
% Both builtins give the same text; nargout's is caught and displayed, and
% nargin's, uncaught on line 9, ends the script.
try
    nargout
catch e
    disp(e.message)
end
nargin
