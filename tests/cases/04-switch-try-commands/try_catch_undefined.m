% covers: 7 - an interpreter error is caught like any other, with its message in e.message
% The wording is the one cycle 01e set.
try, undefined_thing + 1, catch e, disp(e.message), end
