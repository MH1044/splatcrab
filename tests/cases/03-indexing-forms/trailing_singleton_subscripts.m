% covers: 15 - trailing singleton subscripts read and write a 2-D matrix (QA D22)
A = [1 2; 3 4];
disp(A(2, 1, 1))
disp(A(:, :, 1))
A(1, 2, 1) = 9;
disp(A)
