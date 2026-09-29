% covers: 9 - load -ascii refuses a value that is not a number and a line with a different number of values from the lines before it, each a clean error naming the file and the line, exit 1
% The texts are SplatCrab's own. The file is named after the case and deleted
% before the last error is raised again, and after the try as well.
f = 'err_load_ascii_faults.txt';
fid = fopen(f, 'w'); fprintf(fid, '1 2\n3 x\n'); fclose(fid);
try, load(f, '-ascii'); catch e, disp(e.message); end
fid = fopen(f, 'w'); fprintf(fid, '1 2\n3\n'); fclose(fid);
try, load(f, '-ascii'); catch e, delete(f); rethrow(e); end
delete(f);
