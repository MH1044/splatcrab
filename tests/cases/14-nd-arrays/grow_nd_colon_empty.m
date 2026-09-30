% covers: 6 - a colon over an empty target takes the right-hand side's extent in every position, so assigning an N-D array to x(:, :, :) of [] builds it
x = [];
x(:, :, :) = reshape(1:8, 2, 2, 2);
disp(size(x))
disp(x(:)')
