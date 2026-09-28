% covers: 6 - svd returns the singular values in descending order
% The singular values of [3 0; 0 4] are 4 and 3. They are printed unsorted,
% so the descending order is part of what the case asserts.
fprintf('%.4f %.4f\n', svd([3 0; 0 4]));
