% covers: 12 - save of the whole workspace when it holds an N-D array is refused by name and leaves no file behind; exit 1
x = 1;
A = zeros(2, 2, 2);
try
  save('scratch_nd_workspace.mat');
catch e
end
fid = fopen('scratch_nd_workspace.mat');
if fid >= 0
  fclose(fid);
  delete('scratch_nd_workspace.mat');
end
disp(fid)
rethrow(e)
