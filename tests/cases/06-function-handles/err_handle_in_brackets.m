% covers: 12 - [@(x) x+1] is a parse error, and a clean one: nothing runs, exit 1 (the rest of item 12 is unit tests)
% The text is SplatCrab's own, pinned here: MATLAB's as recalled, not
% MATLAB-sourced. Line 6 counts this covers line as line 1. The empty
% output shows the script was refused whole, before disp(1) could run.
disp(1)
v = [@(x) x+1]
