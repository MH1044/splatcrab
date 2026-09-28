% covers: 8 - logical(NaN) is an error
% The disp after it proves the script stops: on the bug it would print 7.
logical(NaN)
disp(7)
