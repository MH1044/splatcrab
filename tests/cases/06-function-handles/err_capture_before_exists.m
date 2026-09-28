% covers: 14 - capture happens at creation: g does not exist yet when @(n) g(n) is made, so inside the body g is looked up as a function, and there is none
% Lines count this covers line as line 1. The handle is made and called on
% line 5, so the Error: prefix names line 5. The trace line that names the
% anonymous function is SplatCrab's own choice, and is not asserted.
g = @(n) g(n); g(1)
