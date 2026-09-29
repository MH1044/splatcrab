% covers: 6 - a UTF-16 script, little-endian with a byte-order mark or without one and big-endian with one, is decoded and runs
% Each file holds disp(7) and a newline, is written byte by byte with fwrite,
% is named after the case, and is deleted before any error is raised again.
t = [double('disp(7)') 10];
le = zeros(1, 16); le(1:2:end) = t;
be = zeros(1, 16); be(2:2:end) = t;
names = {'utf16_source_files_le_bom.m', 'utf16_source_files_le.m', 'utf16_source_files_be_bom.m'};
bytes = {[255 254 le], le, [254 255 be]};
try
  for k = 1:3
    fid = fopen(names{k}, 'w'); fwrite(fid, bytes{k}, 'uint8'); fclose(fid);
  end
  for k = 1:3
    run(names{k});
  end
catch e
  for k = 1:3
    try, delete(names{k}); catch, end
  end
  rethrow(e);
end
for k = 1:3
  delete(names{k});
end
