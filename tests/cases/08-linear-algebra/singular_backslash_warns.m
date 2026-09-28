% covers: 10 - a singular square system warns on stderr instead of erroring, returns a result, and the script runs on to exit 0
% The spec fixes the warning and the exit code but not the entries of the
% result, which depend on the elimination, so the case shows by its size that
% there is one.
x = [1 2; 2 4] \ [1; 2];
disp(size(x))
