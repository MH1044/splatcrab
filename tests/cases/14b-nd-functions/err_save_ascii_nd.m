% covers: 14 - save -ascii of an N-D array is still refused by name, since the text format has rows alone, and leaves no file behind; exit 1
% The refusal is caught so the case can show no file was written, then raised again.
A = reshape(1:24, 2, 3, 4);
try
  save('scratch_nd.txt', 'A', '-ascii');
catch e
end
fid = fopen('scratch_nd.txt');
if fid >= 0
  fclose(fid);
  delete('scratch_nd.txt');
end
disp(fid)
rethrow(e)
