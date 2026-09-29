% covers: 16 - a MAT file whose array header claims 1e12 elements is a clean error on load, exit 1: its dimensions go through check_shape before anything is allocated
% The bytes: the 128-byte header of a little-endian MAT version 5 file, then
% one well-formed miMATRIX element, a double named x whose dimensions say 1e6
% by 1e6 while its real part holds one value. The file is named after the case
% and deleted before the error is raised again, and after the try as well, in
% case load wrongly succeeds. The text is check_shape's: the dimensions come
% before the data, so the spec's rule makes that the first check to fail.
u32 = @(v) mod(floor(v ./ 256 .^ (0:3)), 256);
hdr = [double('MATLAB 5.0 MAT-file') 32 * ones(1, 97) zeros(1, 8) 0 1 double('IM')];
flags = [u32(6) u32(8) u32(6) u32(0)];
dims = [u32(5) u32(8) u32(1e6) u32(1e6)];
name = [u32(1) u32(1) double('x') zeros(1, 7)];
re = [u32(9) u32(8) 0 0 0 0 0 0 240 63];
el = [flags dims name re];
bytes = [hdr u32(14) u32(numel(el)) el];
fid = fopen('err_load_mat_huge_dims.mat', 'w');
fwrite(fid, bytes);
fclose(fid);
try, load('err_load_mat_huge_dims.mat'); catch e, delete('err_load_mat_huge_dims.mat'); rethrow(e); end
delete('err_load_mat_huge_dims.mat');
