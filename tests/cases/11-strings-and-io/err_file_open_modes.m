% covers: 8 - a read from a file open only for writing, a write to one open only for reading and an unknown fread or fwrite precision are clean errors; fopen of a missing file or of a folder is -1 and a reason, never an error
% The texts are SplatCrab's own, and fopen's reasons read the same on every
% platform. The file is named after the case and deleted before the last
% error is raised again.
fid = fopen('err_file_open_modes.txt', 'w');
try, fgetl(fid); catch e, disp(e.message); end
try, fwrite(fid, 1, 'int9'); catch e, disp(e.message); end
fclose(fid);
fid = fopen('err_file_open_modes.txt', 'r');
try, fread(fid, 1, 'int9'); catch e, disp(e.message); end
[f, msg] = fopen('err_file_open_modes_missing.txt'); disp(f); disp(msg);
[f, msg] = fopen('.'); disp(f); disp(msg);
try, fprintf(fid, 'x'); catch e, fclose(fid); delete('err_file_open_modes.txt'); rethrow(e); end
