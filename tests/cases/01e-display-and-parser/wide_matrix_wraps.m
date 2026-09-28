% covers: 10 - a wide matrix wraps into `Columns N through M` blocks
% MATLAB's command window is 80 characters wide and the display fits as many
% whole columns into it as it can. A fixed-point column is 10 characters wide,
% so 8 of them fit exactly; an integer column is 6, so 13 fit (78) and a 14th
% (84) does not. Both widths are pinned here, because one rule has to produce
% both. linspace(1, 2) prints roughly 1300 characters on a single line today.
%
% The rule does not depend on the output being a terminal. The golden harness
% always pipes, so a wrap that switched itself off when piped could not be
% tested at all, and MATLAB's own width does not follow the pipe either.
%
% The values are 1 + k/99 for k = 0..99, rounded to four decimals: k/99 is
% 0.abab... for the two digits of k, so the four decimals are "abab", one ulp
% higher when k >= 50 (the tail exceeds a half). k = 99 carries into 2.0000.
linspace(1, 2)
disp(1:30)
