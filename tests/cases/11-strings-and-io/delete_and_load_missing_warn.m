% covers: 18 - delete of a file that is not there is a warning and the other files named are still deleted; load of a variable the file does not hold is a warning; the script runs on, exit 0
% The texts are SplatCrab's own. Every file is named after the case and all of
% them are gone at the end: the last fopen shows the .txt was deleted.
x = 1;
save('delete_and_load_missing_warn.mat', 'x');
fid = fopen('delete_and_load_missing_warn.txt', 'w'); fclose(fid);
clear x
load('delete_and_load_missing_warn.mat', 'x', 'q');
disp(x);
delete('delete_and_load_missing_warn_gone.txt', 'delete_and_load_missing_warn.txt', 'delete_and_load_missing_warn.mat');
fid = fopen('delete_and_load_missing_warn.txt'); disp(fid);
