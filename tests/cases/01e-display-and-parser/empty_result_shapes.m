% covers: 12 - empty results take MATLAB's shapes, and disp([]) prints nothing
% find([]) and diag([]) are 0x0 in MATLAB, not 0x1; size('') is 0 0, not 1 0;
% and s(:) is a column whatever s is, so a char gives 3x1 rather than 1x3.
%
% The last two lines belong together: disp([]) prints nothing at all, where
% this interpreter prints `     []`, and the 99 after it is the sentinel that
% proves it. Without a line after it, a blank line printed by disp([]) would
% be dropped as trailing whitespace and the case would pass on the bug.
disp(size(find([])))
disp(size(diag([])))
disp(size(''))
s = 'abc';
disp(size(s(:)))
disp([])
disp(99)
