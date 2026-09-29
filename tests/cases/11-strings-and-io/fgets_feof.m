% covers: 8 - (Scope, fgets and feof) fgets keeps the newline fgetl drops and returns -1 at the end; feof is 0 with text left and 1 once a read reaches the end
% The file holds ab, a newline, and cd with no newline after it, so feof is 0
% after the first line however the end is detected, and 1 after the second.
% The file is named after the case and deleted before the case ends.
fid = fopen('fgets_feof.txt', 'w');
fprintf(fid, 'ab\ncd');
fclose(fid);
fid = fopen('fgets_feof.txt');
a = fgets(fid);
disp(double(a))
fprintf('%d\n', feof(fid))
b = fgets(fid);
disp(b)
fprintf('%d\n', feof(fid))
c = fgets(fid);
disp(c)
fclose(fid);
delete('fgets_feof.txt')
