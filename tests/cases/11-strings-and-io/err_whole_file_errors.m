% covers: 10 - fileread, readmatrix, writematrix, csvread and delete refuse an empty name, a missing file, a folder, a value they cannot write and a field that is not a number, each with a clean error naming the file, exit 1
% The texts are SplatCrab's own, with reasons that read the same on every
% platform. The one file written is named after the case and deleted before
% the last error is raised again, and after the try as well.
try, fileread(''); catch e, disp(e.message); end
try, fileread('err_whole_file_missing.txt'); catch e, disp(e.message); end
try, fileread('.'); catch e, disp(e.message); end
try, readmatrix('err_whole_file_missing.txt'); catch e, disp(e.message); end
try, writematrix({1}, 'err_whole_file_errors.csv'); catch e, disp(e.message); end
try, writematrix(1, 'err_whole_file_no_dir/m.csv'); catch e, disp(e.message); end
try, delete('.'); catch e, disp(e.message); end
fid = fopen('err_whole_file_errors.csv', 'w'); fprintf(fid, '1,2\n3,x\n'); fclose(fid);
try, csvread('err_whole_file_errors.csv'); catch e, delete('err_whole_file_errors.csv'); rethrow(e); end
delete('err_whole_file_errors.csv');
