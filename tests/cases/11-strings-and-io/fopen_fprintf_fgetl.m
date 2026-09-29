% covers: 8 - fopen for writing, fprintf to its file id and fclose; then fopen for reading and fgetl line by line, with -1 past the last line
% The file is named after the case, in the case's own directory, and deleted
% before the case ends.
fid = fopen('fopen_fprintf_fgetl.txt', 'w');
fprintf(fid, 'line1\nline2\n');
fclose(fid);
fid = fopen('fopen_fprintf_fgetl.txt');
l = fgetl(fid);
disp(l)
l = fgetl(fid);
l = fgetl(fid);
disp(l)
fclose(fid);
delete('fopen_fprintf_fgetl.txt')
