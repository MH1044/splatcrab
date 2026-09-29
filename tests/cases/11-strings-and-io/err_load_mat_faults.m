% covers: 16 - load refuses a file that is not a version 5 MAT-file, compressed data, an unknown data type, more than two dimensions, a sparse array, an object and cells nested past the bound, each a clean error naming the file, exit 1
% The bytes of each file are built here, little-endian. The last nests 202
% cells, past the reader's bound of 200. The one file is named after the case,
% deleted after each load, and the last line loads it once it is gone.
u32 = @(v) mod(floor(v ./ 256 .^ (0:3)), 256);
hdr = [double('MATLAB 5.0 MAT-file') 32 * ones(1, 97) zeros(1, 8) 0 1 double('IM')];
flags = @(cls) [u32(6) u32(8) u32(cls) u32(0)];
dims11 = [u32(5) u32(8) u32(1) u32(1)];
nm = [u32(1) u32(1) double('x') zeros(1, 7)];
files = {repmat(double('x'), 1, 200), ...
  [hdr u32(15) u32(8) zeros(1, 8)], ...
  [hdr u32(99) u32(8) zeros(1, 8)], ...
  [hdr u32(14) u32(64) flags(6) u32(5) u32(12) u32(1) u32(1) u32(2) u32(0) nm u32(9) u32(0)], ...
  [hdr u32(14) u32(48) flags(5) dims11 nm], ...
  [hdr u32(14) u32(48) flags(3) dims11 nm]};
el = [u32(14) u32(56) flags(6) dims11 u32(1) u32(0) u32(9) u32(8) zeros(1, 8)];
for k = 1:202
  body = [flags(1) dims11 u32(1) u32(0) el];
  el = [u32(14) u32(numel(body)) body];
end
files{end + 1} = [hdr el];
f = 'err_load_mat_faults.mat';
for k = 1:numel(files)
  fid = fopen(f, 'w'); fwrite(fid, files{k}); fclose(fid);
  try, load(f); disp('loaded'); catch e, disp(e.message); end
  delete(f);
end
load(f)
