% covers: 5 - transposing a function handle keeps its refusal, the message naming the class function_handle; exit 1
% The text is the spec's (Scope, cells and structs transpose). disp runs
% first, so the error comes at run time; the Error: prefix counts this
% covers line as line 1.
f = @sin; disp(class(f))
f'
