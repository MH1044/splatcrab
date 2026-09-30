% covers: 12 - save of a variable holding an N-D array is refused by name and leaves no file behind; exit 1
% The refusal is caught so the case can show no file was written, then raised again.
A = zeros(2, 2, 2);
try
  save('scratch_nd.mat', 'A');
catch e
end
fid = fopen('scratch_nd.mat');
if fid >= 0
  fclose(fid);
  delete('scratch_nd.mat');
end
disp(fid)
rethrow(e)
