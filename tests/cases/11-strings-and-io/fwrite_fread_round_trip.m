% covers: 8 - (Scope, fwrite and fread) fwrite writes bytes and returns their count; fread reads them back as a double column
% The file is named after the case and deleted before the case ends.
fid = fopen('fwrite_fread_round_trip.bin', 'w');
n = fwrite(fid, [65 66 67 255]);
fclose(fid);
fprintf('%d\n', n)
fid = fopen('fwrite_fread_round_trip.bin');
d = fread(fid);
fclose(fid);
disp(size(d))
disp(d')
delete('fwrite_fread_round_trip.bin')
