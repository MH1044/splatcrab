% covers: 10 - a page header writes every index past the second, and the pages of a 4-D array come in column-major page order
x = zeros(1, 1, 1, 2)
w = reshape(1:4, 1, 1, 2, 2)
