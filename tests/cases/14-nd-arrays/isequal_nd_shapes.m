% covers: 14 - isequal compares every dimension before any element, so arrays of other shapes are unequal whichever is the larger, and equal N-D arrays compare element by element
disp(isequal(zeros(2, 2, 2), zeros(2, 2, 2)))
disp(isequal(zeros(2, 2, 2), zeros(2, 4)))
disp(isequal(zeros(2, 2, 2), zeros(2, 2)))
disp(isequal(zeros(2, 2), zeros(2, 2, 2)))
disp(isequal(zeros(1, 1, 2), zeros(1, 2)))
A = reshape(1:8, 2, 2, 2);
B = A;
disp(isequal(A, B))
B(2, 2, 2) = 0;
disp(isequal(A, B))
