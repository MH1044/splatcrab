% covers: whitespace inside brackets separates elements, with signs, parens and transpose
a = [1; 2];
b = [3; 4];
x = 5;
fprintf('%d %d\n', size([1 -2]));
fprintf('%d %d\n', size([1 - 2]));
fprintf('%d %d\n', size([1 +2]));
fprintf('%d %d\n', size([1 + 2]));
fprintf('%d %d\n', size([1 (2)]));
fprintf('%d %d\n', size([x -1]));
fprintf('%d %d\n', size([sin(0) -1]));
fprintf('%d %d\n', size([a' b']));
disp([1 -2 + 3])
disp([1 - 2 + 3])
disp([[1 2] -3])
disp([1, -2])
