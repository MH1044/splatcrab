% covers: 16 - a MAT file cut short after its header, inside its first data element, is a clean error on load, exit 1, never a panic or an abort
% The bytes: the 128-byte header of a little-endian MAT version 5 file, then a
% miMATRIX tag claiming 64 bytes, of which only the 16-byte array flags follow.
% The file is named after the case and deleted before the error is raised
% again, and after the try as well, in case load wrongly succeeds.
u32 = @(v) mod(floor(v ./ 256 .^ (0:3)), 256);
hdr = [double('MATLAB 5.0 MAT-file') 32 * ones(1, 97) zeros(1, 8) 0 1 double('IM')];
bytes = [hdr u32(14) u32(64) u32(6) u32(8) u32(6) u32(0)];
fid = fopen('err_load_mat_truncated.mat', 'w');
fwrite(fid, bytes);
fclose(fid);
try, load('err_load_mat_truncated.mat'); catch e, delete('err_load_mat_truncated.mat'); rethrow(e); end
delete('err_load_mat_truncated.mat');
