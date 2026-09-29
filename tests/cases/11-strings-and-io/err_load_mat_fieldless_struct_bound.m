% covers: 16 - a struct array with no fields in a MAT file is bounded by its element count, since its bytes bound nothing: 1x1048576 loads, and 200 bytes claiming 16384x16384 are a clean error at once rather than gigabytes, exit 1
% The bytes: the 128-byte header of a little-endian MAT version 5 file, then
% one miMATRIX element, a struct named s with its dimensions, a field-name
% length of 32 as a small element and a field-names element of no bytes: no
% fields and so no values. The bound is mat::MAX_FIELDLESS, 2^20. The one
% file is named after the case, deleted after each load, and deleted before
% the error is raised again.
u32 = @(v) mod(floor(v ./ 256 .^ (0:3)), 256);
hdr = [double('MATLAB 5.0 MAT-file') 32 * ones(1, 97) zeros(1, 8) 0 1 double('IM')];
fieldless = @(r, c) [u32(6) u32(8) u32(2) u32(0) u32(5) u32(8) u32(r) u32(c) u32(1) u32(1) double('s') zeros(1, 7) u32(5 + 4 * 65536) u32(32) u32(1) u32(0)];
f = 'err_load_mat_fieldless_struct_bound.mat';
el = fieldless(1, 1048576);
fid = fopen(f, 'w'); fwrite(fid, [hdr u32(14) u32(numel(el)) el]); fclose(fid);
S = load(f);
delete(f);
fprintf('%d %d\n', size(S.s));
disp(numel(fieldnames(S.s)))
el = fieldless(16384, 16384);
bytes = [hdr u32(14) u32(numel(el)) el];
disp(numel(bytes))
fid = fopen(f, 'w'); fwrite(fid, bytes); fclose(fid);
tic
try, load(f); catch e, delete(f); disp(toc < 5); rethrow(e); end
delete(f);
