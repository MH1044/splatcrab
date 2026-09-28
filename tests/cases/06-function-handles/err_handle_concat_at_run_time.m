% covers: 12 - (Design notes, Concatenation) a handle variable as a bracket element is refused when the bracket is evaluated: a handle is one function, not an array
% The parser cannot see that f holds a handle, so this is hcat's refusal,
% not the parse error of err_handle_in_brackets, and disp(1) runs first.
% The text is SplatCrab's own, MATLAB's as recalled, not MATLAB-sourced.
% Line 7 counts this covers line as line 1.
f = @sin; disp(1)
v = [f 1]
