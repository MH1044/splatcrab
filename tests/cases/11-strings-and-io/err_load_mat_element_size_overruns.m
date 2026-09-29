% covers: 16 - (Scope, a corrupt file) a MAT file whose data sub-element claims four gigabytes inside a 64-byte array is a clean error on load, exit 1: no length read from the file is trusted or allocated
% The bytes: the 128-byte header of a little-endian MAT version 5 file, then
% one complete miMATRIX element of 64 bytes, a 1x1 double named x, whose real
% part's tag claims 4294967288 bytes while 8 follow. 4294967288 is 2^32 - 8,
% so a length that is rounded up to 8 in 32 bits wraps to 0. The file is
% named after the case and deleted before the error is raised again, and
% after the try as well, in case load wrongly succeeds.
u32 = @(v) mod(floor(v ./ 256 .^ (0:3)), 256);
hdr = [double('MATLAB 5.0 MAT-file') 32 * ones(1, 97) zeros(1, 8) 0 1 double('IM')];
flags = [u32(6) u32(8) u32(6) u32(0)];
dims = [u32(5) u32(8) u32(1) u32(1)];
name = [u32(1) u32(1) double('x') zeros(1, 7)];
re = [u32(9) u32(4294967288) 0 0 0 0 0 0 240 63];
el = [flags dims name re];
bytes = [hdr u32(14) u32(numel(el)) el];
fid = fopen('err_load_mat_element_size_overruns.mat', 'w');
fwrite(fid, bytes);
fclose(fid);
try, load('err_load_mat_element_size_overruns.mat'); catch e, delete('err_load_mat_element_size_overruns.mat'); rethrow(e); end
delete('err_load_mat_element_size_overruns.mat');
