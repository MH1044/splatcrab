% covers: rethrow lasterr warning assert isequal - assert with no message judges its condition as if does, so a non-scalar with a zero fails with MATLAB's Assertion failed.
% The first assert holds (every element non-zero), so the disp runs.
assert([1 1 1])
disp(1)
assert([1 0 1])
