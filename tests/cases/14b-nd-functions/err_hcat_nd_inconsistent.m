% covers: 11 - a comma joining a 2x3x4 array and a 2x3 matrix, whose third dimensions differ, is refused with the inconsistent-dimensions message; exit 1
A = reshape(1:24, 2, 3, 4);
[A, ones(2, 3)]
