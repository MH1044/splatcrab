% covers: 4 - a matmul result size that overflows is named as asked rather than wrapping
% Both operands are empty and allocate nothing, so the old code reached
% Matrix::filled with rows * cols = 2^64, wrapped to a 4294967296-square
% result, printed that size and then panicked on the next transpose with
% exit 101. The size the user asked for is reported instead, exit 1.
x = zeros(2^32, 0) * zeros(0, 2^32);
disp(size(x))
