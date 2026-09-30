% covers: 7 - deletion with one subscript that is not a colon removes those positions along its dimension, a page, a row across pages or columns across pages; one linear subscript leaves a row
A = reshape(1:24, 2, 3, 4);
A(:, :, 2) = [];
disp(size(A))
disp(A(:, :, 2))
A(1, :, :) = [];
disp(size(A))
disp(A(:)')
C = reshape(1:24, 2, 3, 4);
C(:, [1 3], :) = [];
disp(size(C))
disp(C(:)')
B = reshape(1:8, 2, 2, 2);
B(3) = [];
disp(size(B))
disp(B)
