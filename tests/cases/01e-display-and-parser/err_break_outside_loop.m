% covers: 4 - `break` outside a loop is an error, after the output before it is flushed
% QA D8: today this prints 1, stops the script silently and exits 0, so
% `disp(2)` never runs and nothing says why. Octave gives a parse error,
% "break must appear within a loop"; MATLAB errors too. The wording to add to
% error.rs should follow the family already there ("'end' is only valid inside
% an index expression."):
%   'break' is only valid inside a loop.
% The .err pins only `a loop`, so a MATLAB-flavoured sentence passes too; the
% load-bearing half is exit 1 with the 1 flushed and the 2 never printed.
disp(1)
break
disp(2)
