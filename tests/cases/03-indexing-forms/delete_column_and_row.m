% covers: 5 - deleting a whole column, then a whole row, with a colon subscript
A = [1 2 3; 4 5 6];
A(:, 2) = [];
disp(A)
A(1, :) = [];
disp(A)
