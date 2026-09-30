% covers: 14 - save of an array with a dimension past 2147483647, which a version 5 MAT-file cannot hold, is refused before anything is written
C = zeros(0, 1, 5e9);
try
  save('scratch_big.mat', 'C')
catch e
  disp(exist('scratch_big.mat'))
  rethrow(e)
end
