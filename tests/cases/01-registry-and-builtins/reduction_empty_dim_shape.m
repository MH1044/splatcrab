% covers: 19 - a reduction on an empty with an explicit dimension keeps MATLAB's empty shape
disp(size(sum([], 1)))
disp(size(sum([], 2)))
disp(size(prod([], 1)))
disp(isempty(sum([], 1)))
disp(sum([]))
