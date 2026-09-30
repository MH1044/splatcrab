% covers: 13 - save of a whole workspace holding an N-D array writes it with every dimension, and load reads it back beside the other variables; the file is deleted before the case ends
x = 1;
A = reshape(1:24, 2, 3, 4);
try
  save('scratch_nd_workspace.mat');
  clear
  load('scratch_nd_workspace.mat');
  failed = false;
catch e
  failed = true;
end
fid = fopen('scratch_nd_workspace.mat');
if fid >= 0
  fclose(fid);
  delete('scratch_nd_workspace.mat');
end
if failed
  rethrow(e)
end
disp(x)
disp(size(A))
disp(isequal(A, reshape(1:24, 2, 3, 4)))
