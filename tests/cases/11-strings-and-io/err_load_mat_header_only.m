% covers: 16 - a MAT file that ends right after its 128-byte header, the literal reading of "truncated after its header", is a clean error on load, exit 1
% The bytes: the header of a little-endian MAT version 5 file and nothing
% after it. The file is named after the case and deleted before the error is
% raised again, and after the try as well, in case load wrongly succeeds.
% NOTE: the spec's item 16 makes a file cut off after its header an error.
hdr = [double('MATLAB 5.0 MAT-file') 32 * ones(1, 97) zeros(1, 8) 0 1 double('IM')];
fid = fopen('err_load_mat_header_only.mat', 'w');
fwrite(fid, hdr);
fclose(fid);
try, load('err_load_mat_header_only.mat'); catch e, delete('err_load_mat_header_only.mat'); rethrow(e); end
delete('err_load_mat_header_only.mat');
