% covers: 6 - a linear deletion from a matrix leaves a row vector
% The spec records the 1x3 size. The disp(A) line follows from it and from
% column-major order: A(:) is 1 3 2 4, and removing element 2 leaves 1 2 4.
A = [1 2; 3 4];
A(2) = [];
disp(size(A))
disp(A)
