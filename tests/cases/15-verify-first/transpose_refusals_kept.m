% covers: 5 - (Scope, cells and structs transpose) a function handle keeps its refusal through ', .' and transpose, an MException keeps the same refusal naming its own class, and an N-D array keeps its own message
% The texts are the spec's: the refusal names the value's class, so a
% caught exception gives 'MException' where a handle gives
% 'function_handle'. Each is caught and printed on one line.
f = @sin;
try, y = f'; catch e, disp(e.message); end
try, y = f.'; catch e, disp(e.message); end
try, y = transpose(f); catch e, disp(e.message); end
try, error('boom'); catch x, end
try, y = x'; catch e, disp(e.message); end
try, y = ones(2, 2, 2)'; catch e, disp(e.message); end
try, y = {ones(2, 2, 2)}'; disp(size(y{1})); catch e, disp(e.message); end
