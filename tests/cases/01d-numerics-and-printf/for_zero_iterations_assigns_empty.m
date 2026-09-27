% covers: 13 - a for loop that does not run assigns the empty to its loop variable
% Two halves: a name that already held a value must lose it, and a name that
% did not exist must come into existence.
% NOTE: the exact empty shape MATLAB gives is not settled, so this asserts
% isempty alone and never the size. It uses [] rather than 1:0 for the same
% reason: the empty shape of 1:0 is unsettled too.
k = 7;
for k = []
end
disp(isempty(k))
for fresh_loop_var = []
end
disp(isempty(fresh_loop_var))
