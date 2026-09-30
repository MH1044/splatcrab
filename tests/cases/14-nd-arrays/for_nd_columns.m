% covers: 13 - for over an N-D array iterates the columns of its 2-D fold, numel(A(1,:)) of them, each a column of rows elements
for col = reshape(1:8, 2, 2, 2), disp(col'), end
n = 0;
for c = zeros(2, 3, 4)
  n = n + 1;
end
disp(n)
disp(size(c))
